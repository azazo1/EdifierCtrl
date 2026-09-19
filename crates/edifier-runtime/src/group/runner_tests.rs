use std::sync::{Arc, atomic::{AtomicBool, AtomicUsize, Ordering}};

use async_trait::async_trait;
use edifier_group::message::nonce_to_hex;
use tokio::sync::{mpsc, Mutex, Notify};
use tokio::time::{sleep, timeout};

use super::*;

const MAC: &str = "AA:BB:CC:DD:EE:FF";

#[derive(Clone)]
struct RunnerNet {
    incoming: Arc<Mutex<mpsc::Receiver<Vec<u8>>>>,
    inject: mpsc::Sender<Vec<u8>>,
    sent: Arc<Mutex<Vec<Vec<u8>>>>,
}

impl RunnerNet {
    fn new() -> Self {
        let (inject, incoming) = mpsc::channel(64);
        Self {
            incoming: Arc::new(Mutex::new(incoming)),
            inject,
            sent: Arc::new(Mutex::new(Vec::new())),
        }
    }

    async fn inject(&self, bytes: Vec<u8>) {
        self.inject.send(bytes).await.unwrap();
    }

    async fn messages(&self, key: &GroupKey, gid: GroupId) -> Vec<GroupMessage> {
        let sent = self.sent.lock().await;
        sent.iter().filter_map(|bytes| {
            let datagram: Datagram = serde_json::from_slice(bytes).ok()?;
            open(key, gid, now_ms(), &datagram.envelope).ok()
        }).collect()
    }

    async fn clear(&self) {
        self.sent.lock().await.clear();
    }
}

#[async_trait]
impl GroupNet for RunnerNet {
    async fn send(&self, bytes: &[u8]) -> Result<(), TransportError> {
        self.sent.lock().await.push(bytes.to_vec());
        Ok(())
    }

    async fn recv(&self) -> Result<Vec<u8>, TransportError> {
        self.incoming.lock().await.recv().await.ok_or(TransportError::Closed)
    }
}

struct BlockingAudio {
    connected: AtomicBool,
    started: Notify,
    suppress_calls: AtomicUsize,
    max_active: AtomicUsize,
    active: AtomicUsize,
    suppress_delay: Duration,
}

impl BlockingAudio {
    fn new(suppress_delay: Duration) -> Self {
        Self {
            connected: AtomicBool::new(true),
            started: Notify::new(),
            suppress_calls: AtomicUsize::new(0),
            max_active: AtomicUsize::new(0),
            active: AtomicUsize::new(0),
            suppress_delay,
        }
    }

    fn mark_active(&self) {
        let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
        self.max_active.fetch_max(active, Ordering::SeqCst);
    }

    fn clear_active(&self) {
        self.active.fetch_sub(1, Ordering::SeqCst);
    }
}

#[async_trait]
impl AudioControl for BlockingAudio {
    fn operation_timeout(&self) -> Duration {
        Duration::from_secs(8)
    }

    async fn audio_state(&self, _address: &str) -> Result<AudioState, TransportError> {
        Ok(if self.connected.load(Ordering::SeqCst) {
            AudioState::Connected
        } else {
            AudioState::Disconnected
        })
    }

    async fn connect_audio(&self, _address: &str) -> Result<(), TransportError> {
        self.connected.store(true, Ordering::SeqCst);
        Ok(())
    }

    async fn disconnect_audio(&self, _address: &str) -> Result<(), TransportError> {
        self.connected.store(false, Ordering::SeqCst);
        Ok(())
    }

    async fn suppress_autoreconnect(&self, _address: &str, suppress: bool) -> Result<(), TransportError> {
        self.mark_active();
        if suppress {
            self.suppress_calls.fetch_add(1, Ordering::SeqCst);
            self.started.notify_one();
            sleep(self.suppress_delay).await;
            self.connected.store(false, Ordering::SeqCst);
        }
        self.clear_active();
        Ok(())
    }
}

type RunnerHub = GroupHub<RunnerNet, Arc<BlockingAudio>>;

fn packet(hub: &RunnerHub, from: &str, msg: GroupMessage) -> Vec<u8> {
    serde_json::to_vec(&Datagram {
        from: from.into(),
        envelope: seal(&hub.key, hub.gid, now_ms(), msg).unwrap(),
    }).unwrap()
}

async fn holder(hub: &RunnerHub) {
    hub.set_holding(Some(MAC.into())).await;
    hub.net.clear().await;
}

#[tokio::test]
async fn network_loop_keeps_receiving_and_heartbeating_during_slow_platform_action() {
    let net = RunnerNet::new();
    let audio = Arc::new(BlockingAudio::new(Duration::from_millis(7000)));
    let hub = Arc::new(GroupHub::new("lan", "holder", net.clone(), audio.clone(), None));
    holder(&hub).await;
    let runner_hub = hub.clone();
    let runner = tokio::spawn(async move { runner_hub.run().await });

    net.inject(packet(&hub, "requester", GroupMessage::HandoffRequest {
        headphone: MAC.into(),
        nonce: nonce_to_hex([7; 16]),
        deadline_ms: now_ms() + 10_000,
    })).await;
    timeout(Duration::from_secs(1), audio.started.notified()).await.unwrap();

    let mut events = hub.subscribe();
    let mut remote = hub.peer.lock().await.clone();
    remote.id = "remote".into();
    remote.holding = Some(MAC.into());
    let started = Instant::now();
    for index in 0..3 {
        if index > 0 { sleep(Duration::from_millis(3200)).await; }
        remote.hostname = index.to_string();
        net.inject(packet(&hub, "remote", GroupMessage::Announce { peer: remote.clone() })).await;
        timeout(Duration::from_secs(1), async {
            loop {
                if let RuntimeEvent::Peer(peer) = events.recv().await.unwrap() {
                    if peer.id == "remote" && peer.hostname == index.to_string() { break; }
                }
            }
        }).await.unwrap();
        assert_eq!(hub.peers().await.len(), 1);
    }
    assert!(started.elapsed() > PEER_TTL);
    assert!(hub.peers.lock().await["remote"].1 > started + PEER_TTL);
    assert_eq!(audio.active.load(Ordering::SeqCst), 1);
    let messages = net.messages(&hub.key, hub.gid).await;
    let heartbeats = messages.iter().filter(|message| matches!(message, GroupMessage::Announce { peer } if peer.holding.is_none())).count();
    assert!(heartbeats >= 3, "heartbeats={heartbeats}, messages={messages:?}");
    assert_eq!(audio.suppress_calls.load(Ordering::SeqCst), 1);
    assert_eq!(audio.max_active.load(Ordering::SeqCst), 1);

    hub.leave().await.unwrap();
    timeout(Duration::from_secs(2), runner).await.unwrap().unwrap().unwrap();
    assert_eq!(audio.active.load(Ordering::SeqCst), 0);
    assert_eq!(Arc::strong_count(&hub), 1);
}

#[tokio::test]
async fn queued_taken_precedes_expired_tick_after_waiting_for_operation_lock() {
    let net = RunnerNet::new();
    let audio = Arc::new(BlockingAudio::new(Duration::ZERO));
    let hub = Arc::new(GroupHub::new("lan", "holder", net.clone(), audio, None));
    let mac = MacAddr::parse(MAC).unwrap();
    let mut operation = hub.operation.lock().await;
    operation.suppressed.insert(mac);
    {
        let mut machine = hub.machine.lock().await;
        machine.has_audio = true;
        machine.on_message(&GroupMessage::HandoffRequest {
            headphone: MAC.into(), nonce: nonce_to_hex([7; 16]), deadline_ms: now_ms() + 100,
        }, now_ms());
    }
    let mut events = hub.subscribe();
    let runner_hub = hub.clone();
    let runner = tokio::spawn(async move { runner_hub.run().await });
    // 先让 worker 选中 tick 并等待 operation, 再入队 Taken.
    sleep(Duration::from_millis(20)).await;
    net.inject(packet(&hub, "requester", GroupMessage::HandoffTaken {
        nonce: nonce_to_hex([7; 16]),
    })).await;
    let mut remote = hub.peer.lock().await.clone();
    remote.id = "remote".into();
    net.inject(packet(&hub, "remote", GroupMessage::Announce { peer: remote })).await;
    timeout(Duration::from_secs(1), async {
        loop {
            if matches!(events.recv().await.unwrap(), RuntimeEvent::Peer(_)) { break; }
        }
    }).await.unwrap();
    sleep(Duration::from_millis(120)).await;
    drop(operation);
    timeout(Duration::from_secs(1), async {
        loop {
            match events.recv().await.unwrap() {
                RuntimeEvent::Handoff(HandoffProgress::Done) => break,
                RuntimeEvent::Handoff(HandoffProgress::Failed(reason)) => panic!("{reason}"),
                _ => {}
            }
        }
    }).await.unwrap();
    assert_eq!(hub.machine.lock().await.phase(), Phase::Idle);
    assert!(hub.operation.lock().await.suppressed.contains(&mac));
    hub.leave().await.unwrap();
    timeout(Duration::from_secs(1), runner).await.unwrap().unwrap().unwrap();
}

#[tokio::test]
async fn queued_control_actions_are_processed_serially() {
    let net = RunnerNet::new();
    let audio = Arc::new(BlockingAudio::new(Duration::from_millis(80)));
    let hub = Arc::new(GroupHub::new("lan", "holder", net.clone(), audio.clone(), None));
    holder(&hub).await;
    let runner_hub = hub.clone();
    let runner = tokio::spawn(async move { runner_hub.run().await });

    for nonce in [[7u8; 16], [8u8; 16]] {
        net.inject(packet(&hub, "requester", GroupMessage::HandoffRequest {
            headphone: MAC.into(),
            nonce: nonce_to_hex(nonce),
            deadline_ms: now_ms() + 10_000,
        })).await;
    }
    sleep(Duration::from_millis(250)).await;
    assert_eq!(audio.max_active.load(Ordering::SeqCst), 1);
    assert_eq!(audio.suppress_calls.load(Ordering::SeqCst), 1);

    hub.leave().await.unwrap();
    timeout(Duration::from_secs(2), runner).await.unwrap().unwrap().unwrap();
}
