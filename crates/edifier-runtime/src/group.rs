use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use edifier_group::{
    derive_group, open, seal, Action, GroupId, GroupKey, GroupMessage, HandoffMachine,
    HandoffProgress, MacAddr, PeerInfo, Phase,
};
use tokio::sync::{broadcast, watch, Mutex};
use tokio::time::{Duration, Instant};
use tracing::{debug, info, warn};

use crate::cd::CdFallback;
use crate::events::RuntimeEvent;
use crate::net::{Datagram, GroupNet};
use crate::transport::{AudioControl, AudioState, TransportError};

mod actions;
mod runner;
use actions::OperationState;

const PEER_TTL: Duration = Duration::from_secs(6);

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn random_nonce() -> [u8; 16] {
    let mut n = [0u8; 16];
    if getrandom::getrandom(&mut n).is_err() {
        n[..8].copy_from_slice(&now_ms().to_le_bytes());
    }
    n
}

/// 组会话: 发现成员, 执行交接动作.
pub struct GroupHub<N: GroupNet, A: AudioControl> {
    net: N,
    audio: A,
    cd: Option<Arc<dyn CdFallback>>,
    // 状态转换与外部副作用共用此锁, runner 取消后保留待恢复记录.
    operation: Mutex<OperationState>,
    closed: watch::Sender<bool>,
    machine: Mutex<HandoffMachine>,
    gid: GroupId,
    key: GroupKey,
    peer: Mutex<PeerInfo>,
    peers: Mutex<HashMap<String, (PeerInfo, Instant)>>,
    events: broadcast::Sender<RuntimeEvent>,
}

impl<N: GroupNet, A: AudioControl> GroupHub<N, A> {
    pub fn new(
        passphrase: &str,
        local_id: impl Into<String>,
        net: N,
        audio: A,
        cd: Option<Arc<dyn CdFallback>>,
    ) -> Self {
        let (gid, key) = derive_group(passphrase);
        let local_id = local_id.into();
        let machine = HandoffMachine::new(local_id.clone());
        let host = hostname();
        let peer = PeerInfo {
            id: local_id.clone(),
            hostname: if host == "unknown" { local_id } else { host },
            os: std::env::consts::OS.into(),
            can_audio: true,
            holding: None,
            app_version: env!("CARGO_PKG_VERSION").into(),
        };
        let (events, _) = broadcast::channel(64);
        let (closed, _) = watch::channel(false);
        info!(target: "edifier_runtime", id = %peer.id, group = %gid.to_hex(), "加入局域网组");
        Self {
            net,
            audio,
            operation: Mutex::new(OperationState::new(cd.is_some())),
            closed,
            cd,
            machine: Mutex::new(machine),
            gid,
            key,
            peer: Mutex::new(peer),
            peers: Mutex::new(HashMap::new()),
            events,
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<RuntimeEvent> {
        self.events.subscribe()
    }

    pub fn audio(&self) -> &A {
        &self.audio
    }

    pub fn group_id_hex(&self) -> String {
        self.gid.to_hex()
    }

    pub async fn local_id(&self) -> String {
        self.peer.lock().await.id.clone()
    }

    pub async fn peers(&self) -> Vec<PeerInfo> {
        self.peers_at(Instant::now()).await
    }

    async fn peers_at(&self, now: Instant) -> Vec<PeerInfo> {
        let mut peers = self.peers.lock().await;
        peers.retain(|id, (_, seen_at)| {
            let alive = now.saturating_duration_since(*seen_at) < PEER_TTL;
            if !alive {
                info!(target: "edifier_runtime", peer = %id, "组成员心跳超时");
            }
            alive
        });
        peers.values().map(|(peer, _)| peer.clone()).collect()
    }

    pub async fn set_has_audio(&self, has: bool) {
        let _operation = self.operation.lock().await;
        if !*self.closed.borrow() {
            self.machine.lock().await.has_audio = has;
        }
    }

    pub async fn set_can_control(&self, can: bool) {
        let mut operation = self.operation.lock().await;
        if !*self.closed.borrow() {
            operation.can_control = can;
            self.update_control_capability(&operation).await;
        }
    }

    /// 绑定 CD 实际作用的控制通道地址, 与音频持有状态分别管理.
    pub async fn set_control_address(&self, mac: Option<String>) {
        let mut operation = self.operation.lock().await;
        if !*self.closed.borrow() {
            operation.control_address = mac.as_deref().and_then(|s| MacAddr::parse(s).ok());
            self.update_control_capability(&operation).await;
        }
    }

    /// 仅发布经平台确认的音频持有状态, 不发起连接.
    pub async fn set_holding(&self, mac: Option<String>) {
        let mut operation = self.operation.lock().await;
        if *self.closed.borrow() {
            return;
        }
        let mac = mac.as_deref().and_then(|s| MacAddr::parse(s).ok());
        operation.audio_candidate = mac;
        let holding = if let Some(mac) = mac {
            let addr = mac.to_colon_string();
            matches!(
                tokio::time::timeout(Duration::from_secs(3), self.audio.audio_state(&addr)).await,
                Ok(Ok(AudioState::Connected))
            ).then_some(addr)
        } else {
            None
        };
        self.update_holding(holding).await;
        self.announce_inner(&operation).await;
    }

    /// 尝试接管音频, 只有平台确认 Connected 后才认领地址.
    pub async fn adopt_headset(&self, mac: Option<String>) {
        let mut operation = self.operation.lock().await;
        if *self.closed.borrow() {
            return;
        }
        let mac = match mac {
            Some(mac) => match MacAddr::parse(&mac) {
                Ok(mac) => Some(mac),
                Err(err) => {
                    warn!(target: "edifier_runtime", %err, "忽略无效耳机地址");
                    return;
                }
            },
            None => None,
        };
        if self.machine.lock().await.phase() != Phase::Idle {
            debug!(target: "edifier_runtime", "交接期间暂不重新接管音频");
            return;
        }
        operation.audio_candidate = mac;
        let holding = if let Some(mac) = mac {
            let addr = mac.to_colon_string();
            match self.connect_verified(&mut operation, mac).await {
                Ok(_) => Some(addr),
                Err(err) => {
                    warn!(target: "edifier_runtime", %err, address = %addr, "未能确认音频连接");
                    None
                }
            }
        } else {
            None
        };
        self.update_holding(holding).await;
        self.announce_inner(&operation).await;
    }

    pub async fn has_audio(&self) -> bool {
        self.machine.lock().await.has_audio
    }

    pub async fn holding(&self) -> Option<String> {
        // 纯读不等待交接副作用锁, 避免阻塞 UI 的事件轮询和退出请求.
        self.verified_holding().await
    }

    async fn verified_holding(&self) -> Option<String> {
        if *self.closed.borrow() { return None; }
        let addr = self.peer.lock().await.holding.clone()?;
        let connected = matches!(
            tokio::time::timeout(Duration::from_secs(3), self.audio.audio_state(&addr)).await,
            Ok(Ok(AudioState::Connected))
        );
        // 核验期间可能离组或切换目标, 不能发布上一目标的迟到结果.
        if connected && !*self.closed.borrow()
            && self.peer.lock().await.holding.as_ref() == Some(&addr) {
            Some(addr)
        } else {
            None
        }
    }

    pub async fn claim(&self, headphone: &str) -> Result<(), TransportError> {
        self.claim_at(headphone, random_nonce(), now_ms()).await
    }

    pub async fn claim_at(
        &self,
        headphone: &str,
        nonce: [u8; 16],
        now: u64,
    ) -> Result<(), TransportError> {
        let mac = MacAddr::parse(headphone).map_err(|e| TransportError::NotFound(e.to_string()))?;
        let mut operation = self.operation.lock().await;
        self.ensure_open()?;
        {
            let mut machine = self.machine.lock().await;
            if machine.phase() == Phase::Idle {
                operation.audio_candidate = Some(mac);
                machine.can_control_headset = operation.can_control
                    && self.cd.is_some() && operation.control_address == Some(mac);
            }
        }
        info!(target: "edifier_runtime", mac = %mac.to_colon_string(), "请求接管音频");
        let actions = self.machine.lock().await.claim(mac, nonce, now);
        self.apply(&mut operation, actions).await
    }

    pub async fn tick_at(&self, now: u64) {
        let mut operation = self.operation.lock().await;
        self.tick_locked(&mut operation, now).await;
    }

    async fn tick_locked(&self, operation: &mut OperationState, now: u64) {
        if *self.closed.borrow() {
            return;
        }
        let actions = self.machine.lock().await.tick(now);
        if let Err(err) = self.apply(operation, actions).await {
            warn!(target: "edifier_runtime", %err, "交接定时动作失败");
        }
    }

    pub async fn announce(&self) {
        let operation = self.operation.lock().await;
        if !*self.closed.borrow() {
            self.announce_inner(&operation).await;
        }
    }

    /// 离组不主动断音频. 失败的重连抑制恢复会保留, 再次调用时重试.
    pub async fn leave(&self) -> Result<(), TransportError> {
        let mut operation = self.operation.lock().await;
        let was_closed = self.closed.send_replace(true);
        let actions = self.machine.lock().await.on_audio_failed("已离开局域网组");
        self.update_holding(None).await;
        self.peers.lock().await.clear();
        operation.control_address = None;
        operation.audio_candidate = None;
        operation.can_control = false;
        self.machine.lock().await.can_control_headset = false;
        let mut error = None;
        for action in actions {
            match action {
                Action::Send(msg) => {
                    if let Err(err) = self.broadcast(msg).await {
                        error.get_or_insert(err);
                    }
                }
                Action::Report(progress) => {
                    let _ = self.events.send(RuntimeEvent::Handoff(progress));
                }
                _ => {}
            }
        }
        // 不依赖当前 phase: 即使 runner 在平台调用中被 abort 也必须恢复.
        for mac in operation.suppressed.iter().copied().collect::<Vec<_>>() {
            if let Err(err) = self.suppress(&mut operation, mac, false).await {
                error.get_or_insert(err);
            }
        }
        if !was_closed {
            let peer = self.peer.lock().await.clone();
            if let Err(err) = self.broadcast(GroupMessage::Announce { peer }).await {
                error.get_or_insert(err);
            }
        }
        error.map_or(Ok(()), Err)
    }

    fn ensure_open(&self) -> Result<(), TransportError> {
        if *self.closed.borrow() { Err(TransportError::Closed) } else { Ok(()) }
    }

    async fn update_holding(&self, holding: Option<String>) {
        self.machine.lock().await.has_audio = holding.is_some();
        self.peer.lock().await.holding = holding;
    }

    async fn update_control_capability(&self, operation: &OperationState) {
        let mut machine = self.machine.lock().await;
        let target = actions::phase_mac(machine.phase()).or(operation.control_address);
        machine.can_control_headset = operation.can_control && self.cd.is_some()
            && operation.control_address.is_some() && operation.control_address == target;
    }

    async fn announce_inner(&self, operation: &OperationState) {
        let peer = self.refresh_holding(operation).await;
        if let Err(err) = self.broadcast(GroupMessage::Announce { peer }).await {
            warn!(target: "edifier_runtime", %err, "广播组状态失败");
        }
    }

    async fn refresh_holding(&self, operation: &OperationState) -> PeerInfo {
        let mut peer = self.peer.lock().await.clone();
        if self.machine.lock().await.phase() == Phase::Idle {
            // 候选只用于被动观察, 不能为了心跳重连或解除交接留下的重连抑制.
            let candidate = peer.holding.clone()
                .or_else(|| operation.audio_candidate.map(MacAddr::to_colon_string));
            let holding = if let Some(addr) = candidate {
                matches!(
                    tokio::time::timeout(Duration::from_secs(3), self.audio.audio_state(&addr)).await,
                    Ok(Ok(AudioState::Connected))
                ).then_some(addr)
            } else {
                None
            };
            if peer.holding != holding {
                info!(target: "edifier_runtime", holding = holding.as_deref().unwrap_or("-"), "音频持有状态已更新");
            }
            self.update_holding(holding.clone()).await;
            peer.holding = holding;
        } else {
            peer.holding = self.verified_holding().await;
        }
        peer
    }

    async fn broadcast(&self, msg: GroupMessage) -> Result<(), TransportError> {
        let env = seal(&self.key, self.gid, now_ms(), msg)
            .map_err(|err| TransportError::Write(format!("{err:?}")))?;
        let dg = Datagram {
            from: self.peer.lock().await.id.clone(),
            envelope: env,
        };
        let bytes = serde_json::to_vec(&dg)
            .map_err(|err| TransportError::Write(err.to_string()))?;
        tokio::time::timeout(Duration::from_secs(3), self.net.send(&bytes))
            .await.map_err(|_| TransportError::Write("发送组报文超时".into()))?
    }
}

fn hostname() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "unknown".into())
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod runner_tests;
#[cfg(test)]
mod timing_tests;
