use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use async_trait::async_trait;
use edifier_group::message::nonce_to_hex;
use tokio::sync::Notify;
use tokio::time::timeout;

use super::*;

const MAC: &str = "AA:BB:CC:DD:EE:FF";

#[derive(Default)]
struct TimingNet {
    sent: Mutex<Vec<GroupMessage>>,
}

#[async_trait]
impl GroupNet for TimingNet {
    async fn send(&self, bytes: &[u8]) -> Result<(), TransportError> {
        let datagram: Datagram = serde_json::from_slice(bytes).unwrap();
        self.sent.lock().await.push(datagram.envelope.body);
        Ok(())
    }

    async fn recv(&self) -> Result<Vec<u8>, TransportError> {
        std::future::pending().await
    }
}

struct TimingAudio {
    gate: Arc<Mutex<()>>,
    connected: Arc<AtomicBool>,
    suppressed: Arc<AtomicBool>,
    started: Arc<Notify>,
    restore_started: Notify,
    disconnects: AtomicUsize,
    budget: Duration,
    suppress_delay: Duration,
    restore_delay: Duration,
}

impl TimingAudio {
    fn new(budget: Duration, suppress_delay: Duration, restore_delay: Duration) -> Self {
        Self {
            gate: Arc::new(Mutex::new(())),
            connected: Arc::new(AtomicBool::new(true)),
            suppressed: Arc::new(AtomicBool::new(false)),
            started: Arc::new(Notify::new()),
            restore_started: Notify::new(),
            disconnects: AtomicUsize::new(0),
            budget,
            suppress_delay,
            restore_delay,
        }
    }
}

#[async_trait]
impl AudioControl for TimingAudio {
    fn operation_timeout(&self) -> Duration { self.budget }

    async fn audio_state(&self, _address: &str) -> Result<AudioState, TransportError> {
        let Ok(_gate) = self.gate.try_lock() else { return Ok(AudioState::Unknown); };
        Ok(if self.connected.load(Ordering::SeqCst) { AudioState::Connected } else { AudioState::Disconnected })
    }

    async fn connect_audio(&self, _address: &str) -> Result<(), TransportError> {
        self.connected.store(true, Ordering::SeqCst);
        Ok(())
    }

    async fn disconnect_audio(&self, _address: &str) -> Result<(), TransportError> {
        self.disconnects.fetch_add(1, Ordering::SeqCst);
        self.connected.store(false, Ordering::SeqCst);
        Ok(())
    }

    async fn suppress_autoreconnect(&self, _address: &str, suppress: bool) -> Result<(), TransportError> {
        let operation_guard = self.gate.clone().lock_owned().await;
        if suppress {
            let connected = self.connected.clone();
            let suppressed = self.suppressed.clone();
            let started = self.started.clone();
            let delay = self.suppress_delay;
            // 模拟 spawn_blocking: 等待者超时丢弃句柄之后, 原有驱动工作仍运行并持锁.
            tokio::spawn(async move {
                let _gate = operation_guard;
                started.notify_one();
                tokio::time::sleep(delay).await;
                connected.store(false, Ordering::SeqCst);
                suppressed.store(true, Ordering::SeqCst);
            }).await.unwrap();
        } else {
            let _gate = operation_guard;
            self.restore_started.notify_one();
            tokio::time::sleep(self.restore_delay).await;
            self.suppressed.store(false, Ordering::SeqCst);
            self.connected.store(true, Ordering::SeqCst);
        }
        Ok(())
    }
}

type TimingHub = GroupHub<TimingNet, Arc<TimingAudio>>;

async fn holder(audio: Arc<TimingAudio>) -> Arc<TimingHub> {
    let hub = Arc::new(GroupHub::new("lan", "holder", TimingNet::default(), audio, None));
    hub.set_holding(Some(MAC.into())).await;
    hub.net.sent.lock().await.clear();
    hub
}

async fn request(hub: &TimingHub, budget_ms: u64) {
    let body = GroupMessage::HandoffRequest {
        headphone: MAC.into(),
        nonce: nonce_to_hex([7; 16]),
        deadline_ms: now_ms() + budget_ms,
    };
    let bytes = serde_json::to_vec(&Datagram {
        from: "requester".into(),
        envelope: seal(&hub.key, hub.gid, now_ms(), body).unwrap(),
    }).unwrap();
    hub.dispatch(&bytes).await;
}

#[tokio::test]
async fn slow_suppression_uses_platform_budget_without_repeating_disconnect() {
    let audio = Arc::new(TimingAudio::new(Duration::from_secs(8), Duration::from_millis(6400), Duration::ZERO));
    // FFI 使用 Arc<PlatformAudio>, 必须透传平台预算而不是退回 trait 默认 3 秒.
    assert_eq!(audio.operation_timeout(), Duration::from_secs(8));
    let hub = holder(audio.clone()).await;
    timeout(Duration::from_secs(9), request(&hub, 10_000)).await.unwrap();
    let sent = hub.net.sent.lock().await;
    assert!(sent.iter().any(|message| matches!(message, GroupMessage::HandoffReleased { .. })));
    assert!(!sent.iter().any(|message| matches!(message, GroupMessage::HandoffAbort { .. })));
    assert_eq!(audio.disconnects.load(Ordering::SeqCst), 0);
    assert!(!audio.connected.load(Ordering::SeqCst));
    assert!(audio.suppressed.load(Ordering::SeqCst));
}

#[tokio::test]
async fn short_deadline_aborts_but_waits_for_late_driver_and_long_restore() {
    let audio = Arc::new(TimingAudio::new(Duration::from_secs(8), Duration::from_millis(800), Duration::from_millis(3200)));
    let hub = holder(audio.clone()).await;
    let mut events = hub.subscribe();
    let runner_hub = hub.clone();
    let runner = tokio::spawn(async move { request(&runner_hub, 550).await });
    timeout(Duration::from_secs(1), audio.started.notified()).await.unwrap();
    timeout(Duration::from_secs(2), audio.restore_started.notified()).await.unwrap();
    // 超时应通知对端, 但直到先前工作和恢复真正结束, 本机仍持操作锁且不宣告失败已收尾.
    assert!(hub.net.sent.lock().await.iter().any(|message| matches!(message, GroupMessage::HandoffAbort { .. })));
    assert!(!runner.is_finished());
    assert!(hub.operation.try_lock().is_err());
    assert!(audio.suppressed.load(Ordering::SeqCst));
    while let Ok(event) = events.try_recv() {
        assert!(!matches!(event, RuntimeEvent::Handoff(HandoffProgress::Failed(_))));
    }
    timeout(Duration::from_secs(5), runner).await.unwrap().unwrap();
    assert!(audio.connected.load(Ordering::SeqCst));
    assert!(!audio.suppressed.load(Ordering::SeqCst));
    assert!(hub.operation.lock().await.suppressed.is_empty());
    assert_eq!(audio.disconnects.load(Ordering::SeqCst), 0);
    assert!(!hub.net.sent.lock().await.iter().any(|message| matches!(message, GroupMessage::HandoffReleased { .. })));
    assert!(std::iter::from_fn(|| events.try_recv().ok()).any(|event|
        matches!(event, RuntimeEvent::Handoff(HandoffProgress::Failed(_)))));
}
