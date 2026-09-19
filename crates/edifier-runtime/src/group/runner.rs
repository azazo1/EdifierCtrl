//! 网络接收与心跳不等待平台副作用. 控制报文由一个有界队列交给串行 worker.

use tokio::sync::mpsc;
use tokio::time::{interval, MissedTickBehavior};

use super::*;

const CONTROL_QUEUE_CAPACITY: usize = 64;
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(2);
const TICK_INTERVAL: Duration = Duration::from_millis(200);

impl<N: GroupNet, A: AudioControl> GroupHub<N, A> {
    pub async fn pump_one(&self) -> Result<(), TransportError> {
        self.ensure_open()?;
        let bytes = self.net.recv().await?;
        self.dispatch(&bytes).await;
        Ok(())
    }

    pub async fn run(&self) -> Result<(), TransportError> {
        if *self.closed.borrow() {
            return Ok(());
        }
        let (tx, rx) = mpsc::channel(CONTROL_QUEUE_CAPACITY);
        let (stopped, stop) = watch::channel(false);
        // 三个 future 与 run 同作用域, 没有每包 spawn 或丢失 JoinHandle 的后台任务.
        // 接收失败/离组只停止接收新工作, 已开始的 worker 动作仍等待完成.
        let receive = async {
            let result = self.receive_loop(tx).await;
            stopped.send_replace(true);
            result
        };
        let (result, (), ()) = tokio::join!(
            receive,
            self.heartbeat_loop(stop.clone()),
            self.worker_loop(rx, stop),
        );
        result
    }

    async fn receive_loop(&self, tx: mpsc::Sender<GroupMessage>) -> Result<(), TransportError> {
        let mut closed = self.closed.subscribe();
        let mut warned_full = false;
        loop {
            if *closed.borrow() {
                return Ok(());
            }
            let bytes = tokio::select! {
                biased;
                _ = closed.changed() => return Ok(()),
                bytes = self.net.recv() => bytes?,
            };
            let received = Instant::now();
            if let Some(msg) = self.receive_message(&bytes, received).await {
                match tx.try_send(msg) {
                    Ok(()) => warned_full = false,
                    Err(mpsc::error::TrySendError::Full(_)) => {
                        // UDP 本身不保证交付. 满载时丢弃新控制包, 保留已接收控制包的顺序,
                        // 不能在 send().await 上停住整个接收循环而让成员心跳一起过期.
                        if !warned_full {
                            warn!(target: "edifier_runtime", capacity = CONTROL_QUEUE_CAPACITY, "交接控制队列已满, 丢弃新控制报文");
                            warned_full = true;
                        }
                    }
                    Err(mpsc::error::TrySendError::Closed(_)) => return Ok(()),
                }
            }
        }
    }

    async fn heartbeat_loop(&self, mut stopped: watch::Receiver<bool>) {
        let mut closed = self.closed.subscribe();
        let mut hello = interval(HEARTBEAT_INTERVAL);
        hello.set_missed_tick_behavior(MissedTickBehavior::Skip);
        loop {
            if *closed.borrow() || *stopped.borrow() {
                return;
            }
            tokio::select! {
                biased;
                _ = closed.changed() => return,
                _ = stopped.changed() => return,
                _ = hello.tick() => self.announce_cached().await,
            }
        }
    }

    async fn announce_cached(&self) {
        let mut peer = self.peer.lock().await.clone();
        // 不等待 operation 锁, 不触发平台读取. 忙时宁可不声明持有,
        // 也不把一次尚未完成的释放/连接作为新持有状态广播.
        let available = self.operation.try_lock();
        if available.is_err() || *self.closed.borrow() {
            peer.holding = None;
        }
        drop(available);
        if let Err(err) = self.broadcast(GroupMessage::Announce { peer }).await {
            warn!(target: "edifier_runtime", %err, "广播缓存组状态失败");
        }
    }

    async fn worker_loop(&self, mut rx: mpsc::Receiver<GroupMessage>, mut stopped: watch::Receiver<bool>) {
        let mut closed = self.closed.subscribe();
        let mut tick = interval(TICK_INTERVAL);
        tick.set_missed_tick_behavior(MissedTickBehavior::Skip);
        let mut refresh = interval(HEARTBEAT_INTERVAL);
        refresh.set_missed_tick_behavior(MissedTickBehavior::Skip);
        loop {
            if *closed.borrow() || *stopped.borrow() {
                return;
            }
            enum Work { Control(GroupMessage), Tick, Refresh }
            let work = tokio::select! {
                biased;
                _ = closed.changed() => return,
                _ = stopped.changed() => return,
                msg = rx.recv() => match msg { Some(msg) => Work::Control(msg), None => return },
                _ = tick.tick() => Work::Tick,
                _ = refresh.tick() => Work::Refresh,
            };
            let mut operation = self.operation.lock().await;
            if *closed.borrow() || *stopped.borrow() {
                return;
            }
            match work {
                Work::Control(msg) => self.dispatch_control(&mut operation, msg).await,
                work => {
                    // 等待外部操作释放锁期间可能收到 Taken. 拿锁后再检查队列,
                    // 控制消息必须先于逾期 tick, 防止错误回滚已经完成的交接.
                    if let Ok(msg) = rx.try_recv() {
                        self.dispatch_control(&mut operation, msg).await;
                    } else {
                        match work {
                            Work::Tick => self.tick_locked(&mut operation, now_ms()).await,
                            Work::Refresh => {
                                let idle = self.machine.lock().await.phase() == Phase::Idle;
                                if idle {
                                    self.refresh_holding(&operation).await;
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            drop(operation);
            // 让就绪的接收循环先把已到达的控制包入队, 不追赶阻塞期间积累的 tick.
            tokio::task::yield_now().await;
        }
    }

    // pump_one 和已有调用方仍可等待单条控制报文完整执行.
    pub(super) async fn dispatch(&self, bytes: &[u8]) {
        if let Some(msg) = self.receive_message(bytes, Instant::now()).await {
            let mut operation = self.operation.lock().await;
            self.dispatch_control(&mut operation, msg).await;
        }
    }

    async fn receive_message(&self, bytes: &[u8], received: Instant) -> Option<GroupMessage> {
        if *self.closed.borrow() { return None; }
        let dg: Datagram = match serde_json::from_slice(bytes) {
            Ok(value) => value,
            Err(err) => {
                warn!(target: "edifier_runtime", %err, "组报文不是 JSON");
                return None;
            }
        };
        if dg.from == self.peer.lock().await.id || dg.from.is_empty() {
            return None;
        }
        let msg = match open(&self.key, self.gid, now_ms(), &dg.envelope) {
            Ok(msg) => msg,
            Err(err) => {
                debug!(target: "edifier_runtime", ?err, "丢弃组报文");
                return None;
            }
        };
        if let GroupMessage::Announce { mut peer } = msg {
            if peer.id != dg.from {
                warn!(target: "edifier_runtime", sender = %dg.from, peer = %peer.id, "组成员身份与报文来源不一致");
                return None;
            }
            peer.holding = peer.holding.as_deref()
                .and_then(|s| MacAddr::parse(s).ok()).map(MacAddr::to_colon_string);
            let mut peers = self.peers.lock().await;
            if !*self.closed.borrow() {
                peers.insert(peer.id.clone(), (peer.clone(), received));
                let _ = self.events.send(RuntimeEvent::Peer(peer));
            }
            None
        } else {
            Some(msg)
        }
    }

    async fn dispatch_control(&self, operation: &mut OperationState, msg: GroupMessage) {
        if *self.closed.borrow() { return; }
        let now = now_ms();
        if let GroupMessage::HandoffRequest { headphone, nonce, deadline_ms } = &msg {
            let Ok(mac) = MacAddr::parse(headphone) else { return };
            if *deadline_ms <= now || edifier_group::message::parse_nonce(nonce).is_none() {
                return;
            }
            if self.peer.lock().await.holding.as_deref() != Some(&mac.to_colon_string()) {
                if let Err(err) = self.broadcast(GroupMessage::HandoffNoAudio {
                    nonce: nonce.clone(),
                }).await {
                    warn!(target: "edifier_runtime", %err, "发送无音频回复失败");
                }
                return;
            }
            self.machine.lock().await.has_audio = true;
        }
        let actions = self.machine.lock().await.on_message(&msg, now);
        if let Err(err) = self.apply(operation, actions).await {
            warn!(target: "edifier_runtime", %err, "执行交接报文失败");
        }
    }
}
