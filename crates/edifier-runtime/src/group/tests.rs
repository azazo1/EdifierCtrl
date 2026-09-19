use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use async_trait::async_trait;
use edifier_group::{message::nonce_to_hex, HANDOFF_DEADLINE_MS};
use edifier_protocol::AUDIO_CONNECT_GAP_MS;
use tokio::sync::Notify;
use tokio::time::timeout;

use super::*;
use crate::cd::MockCd;

const MAC: &str = "AA:BB:CC:DD:EE:FF";
const OTHER: &str = "11:22:33:44:55:66";

#[derive(Debug, Clone, PartialEq, Eq)]
enum AudioCall {
    Connect(String),
    Disconnect(String),
    SelectOutput(String),
    Suppress(String, bool),
}

struct TestAudio {
    states: Mutex<HashMap<String, AudioState>>,
    calls: Mutex<Vec<AudioCall>>,
    suppressed: Mutex<HashSet<String>>,
    after_connect: AudioState,
    after_disconnect: AudioState,
    disconnect_settles_after: Option<Duration>,
    disconnect_call_delay: Duration,
    disconnect_started: Mutex<Option<Instant>>,
    control_closed: Arc<AtomicBool>,
    unknown_then: Option<AudioState>,
    unknown_reads: AtomicUsize,
    fail_connect: bool,
    fail_select_output: bool,
    fail_suppress: bool,
    unsupported_suppress: bool,
    fail_restore: AtomicBool,
    block_suppress: AtomicBool,
    started: Notify,
    resume: Notify,
}

impl Default for TestAudio {
    fn default() -> Self {
        Self {
            states: Mutex::new(HashMap::new()),
            calls: Mutex::new(Vec::new()),
            suppressed: Mutex::new(HashSet::new()),
            after_connect: AudioState::Connected,
            after_disconnect: AudioState::Disconnected,
            disconnect_settles_after: None,
            disconnect_call_delay: Duration::ZERO,
            disconnect_started: Mutex::new(None),
            control_closed: Arc::new(AtomicBool::new(false)),
            unknown_then: None,
            unknown_reads: AtomicUsize::new(2),
            fail_connect: false,
            fail_select_output: false,
            fail_suppress: false,
            unsupported_suppress: false,
            fail_restore: AtomicBool::new(false),
            block_suppress: AtomicBool::new(false),
            started: Notify::new(),
            resume: Notify::new(),
        }
    }
}

#[async_trait]
impl AudioControl for TestAudio {
    async fn audio_state(&self, address: &str) -> Result<AudioState, TransportError> {
        let started = *self.disconnect_started.lock().await;
        let mut states = self.states.lock().await;
        if let (Some(started), Some(delay)) = (started, self.disconnect_settles_after) {
            if started.elapsed() >= delay {
                states.insert(address.into(), AudioState::Disconnected);
            }
        }
        let current = *states.get(address).unwrap_or(&AudioState::Disconnected);
        if current == AudioState::Unknown {
            if let Some(next) = self.unknown_then {
                if self.unknown_reads.fetch_sub(1, Ordering::SeqCst) == 1 {
                    states.insert(address.into(), next);
                }
            }
        }
        Ok(current)
    }

    async fn connect_audio(&self, address: &str) -> Result<(), TransportError> {
        self.calls.lock().await.push(AudioCall::Connect(address.into()));
        if self.fail_connect {
            return Err(TransportError::Connect("连接失败".into()));
        }
        self.states.lock().await.insert(address.into(), self.after_connect);
        Ok(())
    }

    async fn disconnect_audio(&self, address: &str) -> Result<(), TransportError> {
        self.calls.lock().await.push(AudioCall::Disconnect(address.into()));
        self.control_closed.store(true, Ordering::SeqCst);
        *self.disconnect_started.lock().await = Some(Instant::now());
        self.states.lock().await.insert(address.into(), self.after_disconnect);
        tokio::time::sleep(self.disconnect_call_delay).await;
        Ok(())
    }

    async fn select_output(&self, address: &str) -> Result<(), TransportError> {
        self.calls.lock().await.push(AudioCall::SelectOutput(address.into()));
        if self.fail_select_output {
            Err(TransportError::Unavailable("选择输出失败".into()))
        } else {
            Ok(())
        }
    }

    async fn suppress_autoreconnect(&self, address: &str, suppress: bool) -> Result<(), TransportError> {
        self.calls.lock().await.push(AudioCall::Suppress(address.into(), suppress));
        if suppress {
            if self.unsupported_suppress {
                return Err(TransportError::Unsupported("不支持重连抑制".into()));
            }
            self.suppressed.lock().await.insert(address.into());
            if self.block_suppress.swap(false, Ordering::SeqCst) {
                self.started.notify_one();
                self.resume.notified().await;
            }
            if self.fail_suppress {
                return Err(TransportError::Unavailable("重连抑制失败".into()));
            }
        } else {
            if self.fail_restore.load(Ordering::SeqCst) {
                return Err(TransportError::Unavailable("恢复失败".into()));
            }
            self.suppressed.lock().await.remove(address);
        }
        Ok(())
    }
}

#[derive(Default)]
struct TestNet {
    sent: Mutex<Vec<Vec<u8>>>,
    fail: AtomicBool,
}

#[async_trait]
impl GroupNet for TestNet {
    async fn send(&self, bytes: &[u8]) -> Result<(), TransportError> {
        if self.fail.load(Ordering::SeqCst) {
            return Err(TransportError::Write("网络失败".into()));
        }
        self.sent.lock().await.push(bytes.to_vec());
        Ok(())
    }

    async fn recv(&self) -> Result<Vec<u8>, TransportError> {
        std::future::pending().await
    }
}

struct FailingCd;

#[async_trait]
impl CdFallback for FailingCd {
    async fn send_headset_disconnect(&self) -> Result<(), TransportError> {
        Err(TransportError::Write("CD 失败".into()))
    }
}

struct ClosedControlCd {
    closed: Arc<AtomicBool>,
    calls: AtomicUsize,
    delay: Duration,
}

#[async_trait]
impl CdFallback for ClosedControlCd {
    async fn send_headset_disconnect(&self) -> Result<(), TransportError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        assert!(self.closed.load(Ordering::SeqCst));
        tokio::time::sleep(self.delay).await;
        Err(TransportError::Connect("尚未连接".into()))
    }
}

type Hub = GroupHub<TestNet, TestAudio>;

fn hub(audio: TestAudio, cd: Option<Arc<dyn CdFallback>>) -> Hub {
    GroupHub::new("lan", "local", TestNet::default(), audio, cd)
}

fn request(mac: &str) -> GroupMessage {
    request_with_budget(mac, HANDOFF_DEADLINE_MS)
}

fn request_with_budget(mac: &str, budget_ms: u64) -> GroupMessage {
    GroupMessage::HandoffRequest {
        headphone: mac.into(),
        nonce: nonce_to_hex([7; 16]),
        deadline_ms: now_ms() + budget_ms,
    }
}

async fn dispatch(hub: &Hub, from: &str, msg: GroupMessage) {
    let bytes = serde_json::to_vec(&Datagram {
        from: from.into(),
        envelope: seal(&hub.key, hub.gid, now_ms(), msg).unwrap(),
    }).unwrap();
    hub.dispatch(&bytes).await;
}

async fn messages(hub: &Hub) -> Vec<GroupMessage> {
    hub.net.sent.lock().await.iter().map(|bytes| {
        let dg: Datagram = serde_json::from_slice(bytes).unwrap();
        open(&hub.key, hub.gid, now_ms(), &dg.envelope).unwrap()
    }).collect()
}

async fn seed_holder(hub: &Hub) {
    hub.audio.states.lock().await.insert(MAC.into(), AudioState::Connected);
    hub.adopt_headset(Some("aa-bb-cc-dd-ee-ff".into())).await;
    assert_eq!(hub.holding().await.as_deref(), Some(MAC));
    hub.net.sent.lock().await.clear();
    hub.audio.calls.lock().await.clear();
}

#[tokio::test]
async fn holding_read_does_not_wait_for_handoff_side_effects() {
    let hub = hub(TestAudio::default(), None);
    seed_holder(&hub).await;
    let _operation = hub.operation.lock().await;
    let holding = timeout(Duration::from_millis(100), hub.holding()).await.unwrap();
    assert_eq!(holding.as_deref(), Some(MAC));
    hub.closed.send_replace(true);
    assert_eq!(hub.holding().await, None);
}

#[tokio::test]
async fn adopt_only_holds_verified_connected_audio() {
    for audio in [
        TestAudio { fail_connect: true, ..TestAudio::default() },
        TestAudio { after_connect: AudioState::Unknown, ..TestAudio::default() },
    ] {
        let hub = hub(audio, None);
        hub.adopt_headset(Some("aa-bb-cc-dd-ee-ff".into())).await;
        assert!(!hub.has_audio().await);
        assert_eq!(hub.holding().await, None);
        assert_eq!(hub.audio.calls.lock().await.as_slice(), &[AudioCall::Connect(MAC.into())]);
    }
    let hub = hub(TestAudio::default(), None);
    hub.adopt_headset(Some("aa-bb-cc-dd-ee-ff".into())).await;
    assert!(hub.has_audio().await);
    assert_eq!(hub.holding().await.as_deref(), Some(MAC));
    assert!(!hub.audio.calls.lock().await.iter().any(|c| matches!(c, AudioCall::SelectOutput(_))));
    assert!(matches!(messages(&hub).await.last(), Some(GroupMessage::Announce { peer }) if peer.holding.as_deref() == Some(MAC)));
}

#[tokio::test]
async fn failed_adoption_is_observed_without_reconnecting() {
    let hub = hub(TestAudio { fail_connect: true, ..TestAudio::default() }, None);
    hub.audio.states.lock().await.insert(OTHER.into(), AudioState::Connected);
    hub.adopt_headset(Some("aa-bb-cc-dd-ee-ff".into())).await;
    assert_eq!(hub.peer.lock().await.holding, None);
    let calls = hub.audio.calls.lock().await.clone();
    assert_eq!(calls, vec![AudioCall::Connect(MAC.into())]);

    for state in [AudioState::Unknown, AudioState::Connecting, AudioState::Connected,
        AudioState::Disconnected, AudioState::Connected] {
        hub.audio.states.lock().await.insert(MAC.into(), state);
        hub.announce().await;
        let expected = (state == AudioState::Connected).then_some(MAC);
        assert_eq!(hub.peer.lock().await.holding.as_deref(), expected);
        assert_eq!(hub.holding().await.as_deref(), expected);
        assert_eq!(hub.has_audio().await, expected.is_some());
        assert!(matches!(messages(&hub).await.last(), Some(GroupMessage::Announce { peer })
            if peer.holding.as_deref() == expected));
        assert_eq!(*hub.audio.calls.lock().await, calls);
    }
    hub.set_holding(None).await;
    hub.announce().await;
    assert_eq!(hub.holding().await, None);
    assert_eq!(*hub.audio.calls.lock().await, calls);
}

#[tokio::test]
async fn failed_claim_keeps_candidate_for_manual_connection() {
    let hub = hub(TestAudio { fail_connect: true, ..TestAudio::default() }, None);
    hub.claim_at(MAC, [7; 16], now_ms()).await.unwrap();
    let phase = hub.machine.lock().await.phase();
    hub.audio.states.lock().await.insert(MAC.into(), AudioState::Connected);
    hub.announce().await;
    assert_eq!(hub.machine.lock().await.phase(), phase);
    assert_eq!(hub.peer.lock().await.holding, None);
    assert!(!hub.has_audio().await);
    assert!(hub.audio.calls.lock().await.is_empty());

    hub.audio.states.lock().await.insert(MAC.into(), AudioState::Disconnected);
    dispatch(&hub, "holder", GroupMessage::HandoffReleased { nonce: nonce_to_hex([7; 16]) }).await;
    assert_eq!(hub.machine.lock().await.phase(), Phase::Idle);
    assert_eq!(hub.holding().await, None);
    let calls = hub.audio.calls.lock().await.clone();
    assert_eq!(calls, vec![AudioCall::Connect(MAC.into())]);
    hub.audio.states.lock().await.insert(MAC.into(), AudioState::Connected);
    hub.announce().await;
    assert_eq!(hub.holding().await.as_deref(), Some(MAC));
    assert!(hub.has_audio().await);
    assert_eq!(*hub.audio.calls.lock().await, calls);
    assert!(!messages(&hub).await.iter().any(|m| matches!(m, GroupMessage::HandoffTaken { .. })));
}

#[tokio::test]
async fn released_candidate_is_observed_only_when_idle_without_restoring_suppression() {
    let hub = hub(TestAudio::default(), None);
    seed_holder(&hub).await;
    dispatch(&hub, "requester", request(MAC)).await;
    let phase = hub.machine.lock().await.phase();
    assert!(matches!(phase, Phase::Releasing { .. }));
    let calls = hub.audio.calls.lock().await.clone();
    hub.audio.states.lock().await.insert(MAC.into(), AudioState::Connected);
    hub.announce().await;
    assert_eq!(hub.machine.lock().await.phase(), phase);
    assert_eq!(hub.peer.lock().await.holding, None);
    assert!(!hub.has_audio().await);
    assert!(matches!(messages(&hub).await.last(), Some(GroupMessage::Announce { peer }) if peer.holding.is_none()));

    dispatch(&hub, "requester", GroupMessage::HandoffTaken { nonce: nonce_to_hex([7; 16]) }).await;
    for state in [AudioState::Disconnected, AudioState::Unknown, AudioState::Connecting, AudioState::Connected] {
        hub.audio.states.lock().await.insert(MAC.into(), state);
        hub.announce().await;
        let expected = (state == AudioState::Connected).then_some(MAC);
        assert_eq!(hub.holding().await.as_deref(), expected);
        assert!(matches!(messages(&hub).await.last(), Some(GroupMessage::Announce { peer })
            if peer.holding.as_deref() == expected));
        assert!(hub.operation.lock().await.suppressed.contains(&MacAddr::parse(MAC).unwrap()));
        assert!(hub.audio.suppressed.lock().await.contains(MAC));
        assert_eq!(*hub.audio.calls.lock().await, calls);
    }
}

#[tokio::test]
async fn three_peers_wait_for_holder_even_when_bystander_replies_first() {
    let holder = hub(TestAudio::default(), None);
    let requester = hub(TestAudio::default(), None);
    let bystander = hub(TestAudio::default(), None);
    seed_holder(&holder).await;
    requester.claim_at("aa-bb-cc-dd-ee-ff", [7; 16], now_ms()).await.unwrap();
    let req = messages(&requester).await.pop().unwrap();
    dispatch(&bystander, "requester", req.clone()).await;
    dispatch(&requester, "bystander", messages(&bystander).await.pop().unwrap()).await;
    assert!(requester.audio.calls.lock().await.is_empty());
    assert!(matches!(requester.machine.lock().await.phase(), Phase::WaitingRelease { .. }));
    dispatch(&holder, "requester", req).await;
    for msg in messages(&holder).await {
        dispatch(&requester, "holder", msg).await;
    }
    assert_eq!(requester.holding().await.as_deref(), Some(MAC));
    assert_eq!(holder.holding().await, None);
    assert_eq!(holder.audio.audio_state(MAC).await.unwrap(), AudioState::Disconnected);
    for msg in messages(&requester).await {
        if matches!(msg, GroupMessage::HandoffTaken { .. }) {
            dispatch(&holder, "requester", msg).await;
        }
    }
    assert_eq!(holder.machine.lock().await.phase(), Phase::Idle);
    holder.leave().await.unwrap();
    assert!(holder.audio.suppressed.lock().await.is_empty());
}

#[tokio::test]
async fn requests_for_another_headset_do_not_release_or_send_cd() {
    let cd = Arc::new(MockCd::new());
    let hub = hub(TestAudio::default(), Some(cd.clone()));
    seed_holder(&hub).await;
    hub.set_control_address(Some(MAC.into())).await;
    dispatch(&hub, "requester", request(OTHER)).await;
    assert_eq!(hub.holding().await.as_deref(), Some(MAC));
    assert!(hub.audio.calls.lock().await.is_empty());
    assert_eq!(cd.count(), 0);
    assert!(matches!(messages(&hub).await.as_slice(), [GroupMessage::HandoffNoAudio { .. }]));
}

#[tokio::test]
async fn release_waits_for_system_disconnect_after_control_closes() {
    for transient in [AudioState::Connected, AudioState::Unknown] {
        let audio = TestAudio {
            after_disconnect: transient,
            disconnect_settles_after: Some(Duration::from_millis(150)),
            ..TestAudio::default()
        };
        let cd = Arc::new(ClosedControlCd { closed: audio.control_closed.clone(), calls: AtomicUsize::new(0), delay: Duration::ZERO });
        let hub = hub(audio, Some(cd.clone()));
        seed_holder(&hub).await;
        hub.set_control_address(Some(MAC.into())).await;
        dispatch(&hub, "requester", request(MAC)).await;
        let messages = messages(&hub).await;
        assert!(messages.iter().any(|m| matches!(m, GroupMessage::HandoffReleased { .. })), "系统延迟释放不应中止: {messages:?}");
        assert!(!messages.iter().any(|m| matches!(m, GroupMessage::HandoffAbort { .. })));
        assert_eq!(cd.calls.load(Ordering::SeqCst), 0);
        assert_eq!(hub.holding().await, None);
        assert!(!hub.audio.calls.lock().await.iter().any(|c| matches!(c, AudioCall::Connect(_))));
    }
}

#[tokio::test]
async fn release_still_succeeds_after_closed_control_cd_fails_or_times_out() {
    for cd_delay in [Duration::ZERO, Duration::from_secs(10)] {
        let audio = TestAudio {
            after_disconnect: AudioState::Connected,
            disconnect_settles_after: Some(Duration::from_millis(1400)),
            ..TestAudio::default()
        };
        let cd = Arc::new(ClosedControlCd { closed: audio.control_closed.clone(), calls: AtomicUsize::new(0), delay: cd_delay });
        let hub = hub(audio, Some(cd.clone()));
        seed_holder(&hub).await;
        hub.set_control_address(Some(MAC.into())).await;
        timeout(Duration::from_secs(2), dispatch(&hub, "requester", request_with_budget(MAC, 2000))).await.unwrap();
        assert_eq!(cd.calls.load(Ordering::SeqCst), 1);
        assert_eq!(hub.audio.audio_state(MAC).await.unwrap(), AudioState::Disconnected);
        let messages = messages(&hub).await;
        assert!(messages.iter().any(|m| matches!(m, GroupMessage::HandoffReleased { .. })));
        assert!(!messages.iter().any(|m| matches!(m, GroupMessage::HandoffAbort { .. })));
        assert!(!hub.audio.calls.lock().await.iter().any(|c| matches!(c, AudioCall::Connect(_))));
    }
}

#[tokio::test]
async fn release_uses_request_deadline_without_accepting_connected_or_unknown() {
    for state in [AudioState::Connected, AudioState::Connecting, AudioState::Unknown] {
        let audio = TestAudio { after_disconnect: state, ..TestAudio::default() };
        let cd = Arc::new(ClosedControlCd { closed: audio.control_closed.clone(), calls: AtomicUsize::new(0), delay: Duration::ZERO });
        let hub = hub(audio, Some(cd.clone()));
        seed_holder(&hub).await;
        hub.set_control_address(Some(MAC.into())).await;
        let started = Instant::now();
        timeout(Duration::from_millis(1200), dispatch(&hub, "requester", request_with_budget(MAC, 1200))).await.unwrap();
        // CD 立即失败后仍应完成音频核验, 而不是把关闭控制通道直接视为交接失败或成功.
        assert!(started.elapsed() >= Duration::from_millis(850));
        assert_eq!(cd.calls.load(Ordering::SeqCst), 1);
        assert_eq!(hub.audio.audio_state(MAC).await.unwrap(), state);
        assert_eq!(hub.machine.lock().await.phase(), Phase::Idle);
        assert!(hub.audio.suppressed.lock().await.is_empty());
        let messages = messages(&hub).await;
        assert!(messages.iter().any(|m| matches!(m, GroupMessage::HandoffAbort { .. })));
        assert!(!messages.iter().any(|m| matches!(m, GroupMessage::HandoffReleased { .. })));
    }
}

#[tokio::test]
async fn release_system_timeout_keeps_remaining_observation_budget() {
    let hub = hub(TestAudio {
        after_disconnect: AudioState::Connected,
        disconnect_call_delay: Duration::from_secs(10),
        disconnect_settles_after: Some(Duration::from_millis(3150)),
        ..TestAudio::default()
    }, None);
    seed_holder(&hub).await;
    timeout(Duration::from_secs(4), dispatch(&hub, "requester", request(MAC))).await.unwrap();
    assert!(messages(&hub).await.iter().any(|m| matches!(m, GroupMessage::HandoffReleased { .. })));
    assert_eq!(hub.audio.audio_state(MAC).await.unwrap(), AudioState::Disconnected);
}

#[tokio::test]
async fn release_never_sends_cd_to_another_control_device() {
    let cd = Arc::new(MockCd::new());
    let hub = hub(TestAudio {
        after_disconnect: AudioState::Connected,
        disconnect_settles_after: Some(Duration::from_millis(1000)),
        ..TestAudio::default()
    }, Some(cd.clone()));
    seed_holder(&hub).await;
    hub.set_control_address(Some(OTHER.into())).await;
    dispatch(&hub, "requester", request(MAC)).await;
    assert_eq!(cd.count(), 0);
    assert!(messages(&hub).await.iter().any(|m| matches!(m, GroupMessage::HandoffReleased { .. })));
}

#[tokio::test]
async fn unknown_disconnect_aborts_and_restores_without_released() {
    let hub = hub(TestAudio { after_disconnect: AudioState::Unknown, ..TestAudio::default() }, None);
    seed_holder(&hub).await;
    dispatch(&hub, "requester", request("aa-bb-cc-dd-ee-ff")).await;
    assert_eq!(hub.machine.lock().await.phase(), Phase::Idle);
    assert_eq!(hub.holding().await, None);
    assert_eq!(hub.peer.lock().await.holding.as_deref(), Some(MAC));
    assert!(hub.audio.suppressed.lock().await.is_empty());
    let messages = messages(&hub).await;
    assert!(messages.iter().any(|m| matches!(m, GroupMessage::HandoffAbort { .. })));
    assert!(!messages.iter().any(|m| matches!(m, GroupMessage::HandoffReleased { .. })));
}

#[tokio::test]
async fn transient_unknown_waits_for_verified_connection_and_release() {
    let requester = hub(TestAudio {
        after_connect: AudioState::Unknown,
        unknown_then: Some(AudioState::Connected),
        ..TestAudio::default()
    }, None);
    requester.claim_at(MAC, [7; 16], 0).await.unwrap();
    dispatch(&requester, "holder", GroupMessage::HandoffReleased { nonce: nonce_to_hex([7; 16]) }).await;
    assert_eq!(requester.holding().await.as_deref(), Some(MAC));
    assert!(messages(&requester).await.iter().any(|m| matches!(m, GroupMessage::HandoffTaken { .. })));

    let holder = hub(TestAudio {
        after_disconnect: AudioState::Unknown,
        unknown_then: Some(AudioState::Disconnected),
        ..TestAudio::default()
    }, None);
    seed_holder(&holder).await;
    dispatch(&holder, "requester", request(MAC)).await;
    assert_eq!(holder.holding().await, None);
    assert!(messages(&holder).await.iter().any(|m| matches!(m, GroupMessage::HandoffReleased { .. })));
}

#[tokio::test]
async fn claimed_connect_without_audio_never_reports_taken() {
    let hub = hub(TestAudio { after_connect: AudioState::Unknown, ..TestAudio::default() }, None);
    hub.claim_at(MAC, [7; 16], 0).await.unwrap();
    dispatch(&hub, "holder", GroupMessage::HandoffReleased { nonce: nonce_to_hex([7; 16]) }).await;
    assert_eq!(hub.machine.lock().await.phase(), Phase::Idle);
    assert!(!hub.has_audio().await);
    assert_eq!(hub.holding().await, None);
    let messages = messages(&hub).await;
    assert!(messages.iter().any(|m| matches!(m, GroupMessage::HandoffAbort { .. })));
    assert!(!messages.iter().any(|m| matches!(m, GroupMessage::HandoffTaken { .. })));
}

#[tokio::test]
async fn output_selection_failure_rolls_back_only_new_audio_and_reports_actual_holding() {
    for (already_connected, rollback_fails) in [(false, false), (true, false), (false, true)] {
        let hub = hub(TestAudio {
            fail_select_output: true,
            after_disconnect: if rollback_fails { AudioState::Connected } else { AudioState::Disconnected },
            ..TestAudio::default()
        }, None);
        if already_connected {
            hub.audio.states.lock().await.insert(MAC.into(), AudioState::Connected);
        }
        let mut events = hub.subscribe();
        hub.claim_at(MAC, [7; 16], 0).await.unwrap();
        dispatch(&hub, "holder", GroupMessage::HandoffReleased { nonce: nonce_to_hex([7; 16]) }).await;
        assert_eq!(hub.machine.lock().await.phase(), Phase::Idle);
        let remains_connected = already_connected || rollback_fails;
        assert_eq!(hub.holding().await.as_deref(), remains_connected.then_some(MAC));
        assert_eq!(hub.has_audio().await, remains_connected);
        assert_eq!(hub.audio.audio_state(MAC).await.unwrap(),
            if remains_connected { AudioState::Connected } else { AudioState::Disconnected });
        let expected_calls = if already_connected {
            vec![AudioCall::SelectOutput(MAC.into())]
        } else {
            vec![AudioCall::Connect(MAC.into()), AudioCall::SelectOutput(MAC.into()), AudioCall::Disconnect(MAC.into())]
        };
        assert_eq!(*hub.audio.calls.lock().await, expected_calls);
        let messages = messages(&hub).await;
        assert!(messages.iter().any(|m| matches!(m, GroupMessage::HandoffAbort { .. })));
        assert!(!messages.iter().any(|m| matches!(m, GroupMessage::HandoffTaken { .. })));
        assert!(messages.iter().any(|m| matches!(m, GroupMessage::Announce { peer }
            if peer.holding.as_deref() == remains_connected.then_some(MAC))));
        let mut failed = false;
        while let Ok(event) = events.try_recv() {
            assert!(!matches!(event, RuntimeEvent::Handoff(HandoffProgress::Done)));
            failed |= matches!(event, RuntimeEvent::Handoff(HandoffProgress::Failed(_)));
        }
        assert!(failed);
    }
}

#[tokio::test]
async fn failed_suppression_stops_release_and_rolls_back() {
    let hub = hub(TestAudio { fail_suppress: true, ..TestAudio::default() }, None);
    seed_holder(&hub).await;
    dispatch(&hub, "requester", request(MAC)).await;
    assert_eq!(hub.machine.lock().await.phase(), Phase::Idle);
    assert!(hub.audio.suppressed.lock().await.is_empty());
    assert_eq!(hub.audio.calls.lock().await.as_slice(), &[
        AudioCall::Suppress(MAC.into(), true), AudioCall::Suppress(MAC.into(), false),
    ]);
}

#[tokio::test]
async fn unsupported_suppression_still_verifies_release() {
    let hub = hub(TestAudio { unsupported_suppress: true, ..TestAudio::default() }, None);
    seed_holder(&hub).await;
    dispatch(&hub, "requester", request(MAC)).await;
    assert!(messages(&hub).await.iter().any(|m| matches!(m, GroupMessage::HandoffReleased { .. })));
    assert!(hub.operation.lock().await.suppressed.is_empty());
}

#[tokio::test]
async fn failed_cd_cancels_fallback_and_never_connects() {
    let hub = hub(TestAudio::default(), Some(Arc::new(FailingCd)));
    hub.set_control_address(Some(MAC.into())).await;
    hub.claim_at(MAC, [7; 16], 0).await.unwrap();
    hub.tick_at(HANDOFF_DEADLINE_MS).await;
    assert_eq!(hub.machine.lock().await.phase(), Phase::Idle);
    hub.tick_at(HANDOFF_DEADLINE_MS + AUDIO_CONNECT_GAP_MS).await;
    assert!(hub.audio.calls.lock().await.is_empty());
    assert!(messages(&hub).await.iter().any(|m| matches!(m, GroupMessage::HandoffAbort { .. })));
}

#[tokio::test]
async fn cd_fallback_requires_matching_control_address() {
    let cd = Arc::new(MockCd::new());
    let hub = hub(TestAudio::default(), Some(cd.clone()));
    hub.set_control_address(Some(OTHER.into())).await;
    hub.claim_at(MAC, [7; 16], 0).await.unwrap();
    hub.tick_at(HANDOFF_DEADLINE_MS).await;
    assert_eq!(cd.count(), 0);
    assert_eq!(hub.machine.lock().await.phase(), Phase::Idle);
    hub.set_control_address(Some("aa-bb-cc-dd-ee-ff".into())).await;
    hub.claim_at(MAC, [8; 16], 0).await.unwrap();
    hub.tick_at(HANDOFF_DEADLINE_MS).await;
    assert_eq!(cd.count(), 1);
    hub.tick_at(HANDOFF_DEADLINE_MS + AUDIO_CONNECT_GAP_MS).await;
    assert_eq!(hub.holding().await.as_deref(), Some(MAC));
}

#[tokio::test]
async fn busy_claim_returns_error_without_canceling_original_claim() {
    let hub = hub(TestAudio::default(), Some(Arc::new(MockCd::new())));
    hub.set_control_address(Some(MAC.into())).await;
    hub.claim_at(MAC, [7; 16], 0).await.unwrap();
    let phase = hub.machine.lock().await.phase();
    assert!(hub.claim_at(OTHER, [8; 16], 1).await.is_err());
    assert_eq!(hub.machine.lock().await.phase(), phase);
    assert_eq!(hub.operation.lock().await.audio_candidate, Some(MacAddr::parse(MAC).unwrap()));
    assert!(hub.machine.lock().await.can_control_headset);
    assert_eq!(messages(&hub).await.len(), 1);
}

#[tokio::test]
async fn failed_network_send_resets_claim_and_propagates_error() {
    let hub = hub(TestAudio::default(), None);
    hub.net.fail.store(true, Ordering::SeqCst);
    assert!(hub.claim_at(MAC, [7; 16], 0).await.is_err());
    assert_eq!(hub.machine.lock().await.phase(), Phase::Idle);
    hub.net.fail.store(false, Ordering::SeqCst);
    hub.claim_at(MAC, [8; 16], 1).await.unwrap();
}

#[tokio::test]
async fn leave_recovers_aborted_runner_and_blocks_future_actions() {
    let hub = Arc::new(hub(TestAudio::default(), None));
    seed_holder(&hub).await;
    hub.audio.block_suppress.store(true, Ordering::SeqCst);
    let runner_hub = hub.clone();
    let runner = tokio::spawn(async move { dispatch(&runner_hub, "requester", request(MAC)).await });
    timeout(Duration::from_secs(1), hub.audio.started.notified()).await.unwrap();
    let pending_hub = hub.clone();
    let pending = tokio::spawn(async move { pending_hub.claim_at(OTHER, [8; 16], 0).await });
    tokio::task::yield_now().await;
    assert!(!pending.is_finished());
    runner.abort();
    assert!(runner.await.unwrap_err().is_cancelled());
    assert!(pending.await.unwrap().is_err());
    hub.leave().await.unwrap();
    hub.leave().await.unwrap();
    assert_eq!(hub.machine.lock().await.phase(), Phase::Idle);
    assert!(!hub.has_audio().await);
    assert_eq!(hub.holding().await, None);
    assert!(hub.audio.suppressed.lock().await.is_empty());
    assert!(hub.operation.lock().await.suppressed.is_empty());
    assert_eq!(hub.audio.audio_state(MAC).await.unwrap(), AudioState::Connected);
    assert!(!hub.audio.calls.lock().await.iter().any(|c| matches!(c, AudioCall::Disconnect(_))));
    let calls = hub.audio.calls.lock().await.clone();
    assert!(matches!(hub.claim(MAC).await, Err(TransportError::Closed)));
    hub.adopt_headset(Some(OTHER.into())).await;
    hub.set_holding(Some(MAC.into())).await;
    hub.tick_at(u64::MAX).await;
    dispatch(&hub, "requester", request(MAC)).await;
    assert_eq!(*hub.audio.calls.lock().await, calls);
}

#[tokio::test]
async fn leave_retries_failed_restore_without_touching_other_suppression() {
    let hub = hub(TestAudio::default(), None);
    seed_holder(&hub).await;
    hub.audio.suppressed.lock().await.insert(OTHER.into());
    dispatch(&hub, "requester", request(MAC)).await;
    hub.audio.fail_restore.store(true, Ordering::SeqCst);
    assert!(hub.leave().await.is_err());
    assert_eq!(hub.holding().await, None);
    assert_eq!(hub.machine.lock().await.phase(), Phase::Idle);
    hub.audio.fail_restore.store(false, Ordering::SeqCst);
    hub.leave().await.unwrap();
    assert_eq!(*hub.audio.suppressed.lock().await, HashSet::from([OTHER.to_owned()]));
}

#[tokio::test]
async fn peers_expire_without_heartbeats_and_announcements_renew_them() {
    let hub = hub(TestAudio::default(), None);
    let mut peer = hub.peer.lock().await.clone();
    peer.id = "remote".into();
    peer.holding = Some("aa-bb-cc-dd-ee-ff".into());
    dispatch(&hub, "remote", GroupMessage::Announce { peer: peer.clone() }).await;
    let seen_at = hub.peers.lock().await["remote"].1;
    assert_eq!(hub.peers_at(seen_at + PEER_TTL - Duration::from_millis(1)).await.len(), 1);
    assert!(hub.peers_at(seen_at + PEER_TTL).await.is_empty());
    assert!(hub.peers.lock().await.is_empty());

    dispatch(&hub, "remote", GroupMessage::Announce { peer: peer.clone() }).await;
    let first_seen = hub.peers.lock().await["remote"].1;
    hub.peers.lock().await.get_mut("remote").unwrap().1 = first_seen - PEER_TTL;
    dispatch(&hub, "remote", GroupMessage::Announce { peer }).await;
    let renewed = hub.peers().await;
    assert_eq!(renewed.len(), 1);
    assert_eq!(renewed[0].holding.as_deref(), Some(MAC));
}

#[tokio::test]
async fn holding_queries_and_announcements_revalidate_audio() {
    let hub = hub(TestAudio::default(), None);
    seed_holder(&hub).await;
    for state in [AudioState::Disconnected, AudioState::Unknown] {
        hub.audio.states.lock().await.insert(MAC.into(), state);
        assert_eq!(hub.holding().await, None);
        hub.announce().await;
        assert_eq!(hub.peer.lock().await.holding, None);
        assert!(!hub.has_audio().await);
        assert!(matches!(messages(&hub).await.last(), Some(GroupMessage::Announce { peer }) if peer.holding.is_none()));
    }
    hub.audio.states.lock().await.insert(MAC.into(), AudioState::Connected);
    hub.announce().await;
    assert_eq!(hub.holding().await.as_deref(), Some(MAC));
}

#[tokio::test]
async fn leave_stops_idle_runner_without_abort() {
    let hub = Arc::new(hub(TestAudio::default(), None));
    let runner_hub = hub.clone();
    let runner = tokio::spawn(async move { runner_hub.run().await });
    tokio::task::yield_now().await;
    hub.leave().await.unwrap();
    timeout(Duration::from_secs(1), runner).await.unwrap().unwrap().unwrap();
}
