use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use edifier_group::{
    derive_group, open, seal, Action, GroupId, GroupKey, GroupMessage, HandoffMachine,
    HandoffProgress, MacAddr, PeerInfo,
};
use tokio::sync::{broadcast, Mutex};
use tokio::time::{interval, Duration};
use tracing::{debug, info, warn};

use crate::cd::CdFallback;
use crate::events::RuntimeEvent;
use crate::net::{Datagram, GroupNet};
use crate::transport::{AudioControl, AudioState, TransportError};

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
    machine: Mutex<HandoffMachine>,
    gid: GroupId,
    key: GroupKey,
    peer: Mutex<PeerInfo>,
    peers: Mutex<HashMap<String, PeerInfo>>,
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
        let mut machine = HandoffMachine::new(local_id.clone());
        machine.can_control_headset = cd.is_some();
        let peer = PeerInfo {
            id: local_id,
            hostname: hostname(),
            os: std::env::consts::OS.into(),
            can_audio: true,
            holding: None,
            app_version: env!("CARGO_PKG_VERSION").into(),
        };
        let (events, _) = broadcast::channel(64);
        info!(
            target: "edifier_runtime",
            id = %peer.id,
            group = %gid.to_hex(),
            "加入局域网组"
        );
        Self {
            net,
            audio,
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
        self.peers.lock().await.values().cloned().collect()
    }

    pub async fn set_has_audio(&self, has: bool) {
        self.machine.lock().await.has_audio = has;
    }

    pub async fn set_can_control(&self, can: bool) {
        self.machine.lock().await.can_control_headset = can;
    }

    pub async fn set_holding(&self, mac: Option<String>) {
        info!(
            target: "edifier_runtime",
            holding = mac.as_deref().unwrap_or("-"),
            "更新本机持有的耳机"
        );
        self.peer.lock().await.holding = mac;
        self.announce().await;
    }

    /// 控制通道连上后认领耳机: 已连 A2DP 则记下, 否则尝试连接, 并标 has_audio, 对端 claim 才会让本机断开.
    pub async fn adopt_headset(&self, mac: Option<String>) {
        match mac {
            None => {
                self.machine.lock().await.has_audio = false;
                self.set_holding(None).await;
            }
            Some(mac) => {
                let connected = match self.audio.audio_state(&mac).await {
                    Ok(AudioState::Connected) => true,
                    _ => self.audio.connect_audio(&mac).await.is_ok(),
                };
                if !connected {
                    warn!(
                        target: "edifier_runtime",
                        address = %mac,
                        "未能接管 A2DP, 仍标记持有, claim 时会尝试断开"
                    );
                }
                self.machine.lock().await.has_audio = true;
                self.set_holding(Some(mac)).await;
            }
        }
    }

    pub async fn has_audio(&self) -> bool {
        self.machine.lock().await.has_audio
    }

    pub async fn holding(&self) -> Option<String> {
        self.peer.lock().await.holding.clone()
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
        info!(target: "edifier_runtime", mac = %mac.to_colon_string(), "请求接管音频");
        let actions = self.machine.lock().await.claim(mac, nonce, now);
        self.apply(actions).await;
        Ok(())
    }

    pub async fn tick_at(&self, now: u64) {
        let actions = self.machine.lock().await.tick(now);
        self.apply(actions).await;
    }

    pub async fn announce(&self) {
        let peer = self.peer.lock().await.clone();
        self.broadcast(GroupMessage::Announce { peer }).await;
    }

    pub async fn pump_one(&self) -> Result<(), TransportError> {
        let bytes = self.net.recv().await?;
        self.dispatch(&bytes).await;
        Ok(())
    }

    pub async fn run(&self) -> Result<(), TransportError> {
        self.announce().await;
        let mut tick = interval(Duration::from_millis(200));
        let mut hello = interval(Duration::from_secs(2));
        loop {
            tokio::select! {
                bytes = self.net.recv() => {
                    self.dispatch(&bytes?).await;
                }
                _ = tick.tick() => {
                    self.tick_at(now_ms()).await;
                }
                _ = hello.tick() => {
                    self.announce().await;
                }
            }
        }
    }

    async fn dispatch(&self, bytes: &[u8]) {
        let dg: Datagram = match serde_json::from_slice(bytes) {
            Ok(v) => v,
            Err(err) => {
                warn!(target: "edifier_runtime", %err, "组报文不是 JSON");
                return;
            }
        };
        if dg.from == self.peer.lock().await.id {
            return;
        }
        let now = now_ms();
        let msg = match open(&self.key, self.gid, now, &dg.envelope) {
            Ok(m) => m,
            Err(err) => {
                debug!(target: "edifier_runtime", ?err, "丢弃组报文");
                return;
            }
        };
        if let GroupMessage::Announce { peer } = &msg {
            info!(
                target: "edifier_runtime",
                peer = %peer.id,
                host = %peer.hostname,
                "发现组员"
            );
            self.peers
                .lock()
                .await
                .insert(peer.id.clone(), peer.clone());
            let _ = self.events.send(RuntimeEvent::Peer(peer.clone()));
        }
        let actions = self.machine.lock().await.on_message(&msg, now);
        self.apply(actions).await;
    }

    async fn apply(&self, actions: Vec<Action>) {
        let mut q: VecDeque<Action> = actions.into();
        while let Some(action) = q.pop_front() {
            let more = match action {
                Action::Send(msg) => {
                    self.broadcast(msg).await;
                    Vec::new()
                }
                Action::ConnectAudio(mac) => {
                    let addr = mac.to_colon_string();
                    match self.audio.connect_audio(&addr).await {
                        Ok(()) => {
                            self.peer.lock().await.holding = Some(addr);
                            self.machine.lock().await.on_audio_connected()
                        }
                        Err(err) => self.machine.lock().await.on_audio_failed(&err.to_string()),
                    }
                }
                Action::DisconnectAudio(mac) => {
                    let addr = mac.to_colon_string();
                    if let Err(err) = self.audio.disconnect_audio(&addr).await {
                        warn!(target: "edifier_runtime", %err, "断开音频失败");
                    }
                    self.peer.lock().await.holding = None;
                    self.machine.lock().await.on_audio_disconnected()
                }
                Action::SuppressAutoreconnect { mac, suppress } => {
                    let addr = mac.to_colon_string();
                    if let Err(err) = self
                        .audio
                        .suppress_autoreconnect(&addr, suppress)
                        .await
                    {
                        warn!(target: "edifier_runtime", %err, "抑制重连失败");
                    }
                    Vec::new()
                }
                Action::SendHeadsetDisconnect => {
                    if let Some(cd) = &self.cd {
                        if let Err(err) = cd.send_headset_disconnect().await {
                            warn!(target: "edifier_runtime", %err, "CD 失败");
                        }
                    } else {
                        warn!(target: "edifier_runtime", "没有控制通道, 无法发 CD");
                    }
                    Vec::new()
                }
                Action::Report(progress) => {
                    if matches!(progress, HandoffProgress::Done) {
                        info!(target: "edifier_runtime", "交接完成");
                    }
                    let _ = self.events.send(RuntimeEvent::Handoff(progress));
                    Vec::new()
                }
            };
            for a in more.into_iter().rev() {
                q.push_front(a);
            }
        }
    }

    async fn broadcast(&self, msg: GroupMessage) {
        let env = match seal(&self.key, self.gid, now_ms(), msg) {
            Ok(v) => v,
            Err(err) => {
                warn!(target: "edifier_runtime", ?err, "封装组报文失败");
                return;
            }
        };
        let dg = Datagram {
            from: self.peer.lock().await.id.clone(),
            envelope: env,
        };
        let bytes = match serde_json::to_vec(&dg) {
            Ok(v) => v,
            Err(err) => {
                warn!(target: "edifier_runtime", %err, "序列化组报文失败");
                return;
            }
        };
        if let Err(err) = self.net.send(&bytes).await {
            warn!(target: "edifier_runtime", %err, "发送组报文失败");
        }
    }
}

fn hostname() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "unknown".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cd::MockCd;
    use crate::mock::MockAudio;
    use crate::net::LoopbackNet;
    use crate::transport::AudioState;
    use edifier_protocol::AUDIO_CONNECT_GAP_MS;
    use edifier_group::HANDOFF_DEADLINE_MS;

    const MAC: &str = "11:22:33:44:55:66";

    fn nonce() -> [u8; 16] {
        [7u8; 16]
    }

    #[tokio::test]
    async fn holder_releases_then_peer_takes() {
        let (net_a, net_b) = LoopbackNet::pair();
        let a = GroupHub::new("lan", "host-a", net_a, MockAudio::new(), None);
        let b = GroupHub::new("lan", "host-b", net_b, MockAudio::new(), None);
        a.audio().connect_audio(MAC).await.unwrap();
        a.set_has_audio(true).await;

        b.claim_at(MAC, nonce(), 10).await.unwrap();
        a.pump_one().await.unwrap();
        b.pump_one().await.unwrap();
        b.pump_one().await.unwrap();
        a.pump_one().await.unwrap();

        assert_eq!(
            b.audio().audio_state(MAC).await.unwrap(),
            AudioState::Connected
        );
        assert_eq!(
            a.audio().audio_state(MAC).await.unwrap(),
            AudioState::Disconnected
        );
    }

    #[tokio::test]
    async fn silent_holder_falls_back_to_cd() {
        let (net_a, net_b) = LoopbackNet::pair();
        let cd = Arc::new(MockCd::new());
        let _a = GroupHub::new("lan", "host-a", net_a, MockAudio::new(), None);
        let b = GroupHub::new(
            "lan",
            "host-b",
            net_b,
            MockAudio::new(),
            Some(cd.clone()),
        );
        b.set_can_control(true).await;
        b.claim_at(MAC, nonce(), 0).await.unwrap();
        b.tick_at(HANDOFF_DEADLINE_MS).await;
        assert_eq!(cd.count(), 1);
        b.tick_at(HANDOFF_DEADLINE_MS + AUDIO_CONNECT_GAP_MS).await;
        assert_eq!(
            b.audio().audio_state(MAC).await.unwrap(),
            AudioState::Connected
        );
    }

    #[tokio::test]
    async fn adopt_headset_makes_holder_release() {
        let (net_a, net_b) = LoopbackNet::pair();
        let a = GroupHub::new("lan", "host-a", net_a, MockAudio::new(), None);
        let b = GroupHub::new("lan", "host-b", net_b, MockAudio::new(), None);
        a.adopt_headset(Some(MAC.into())).await;
        assert!(a.has_audio().await);
        b.pump_one().await.unwrap();
        b.claim_at(MAC, nonce(), 10).await.unwrap();
        a.pump_one().await.unwrap();
        b.pump_one().await.unwrap();
        b.pump_one().await.unwrap();
        a.pump_one().await.unwrap();
        assert_eq!(
            b.audio().audio_state(MAC).await.unwrap(),
            AudioState::Connected
        );
        assert_eq!(
            a.audio().audio_state(MAC).await.unwrap(),
            AudioState::Disconnected
        );
    }

    #[tokio::test]
    async fn holding_is_announced_to_peer() {
        let (net_a, net_b) = LoopbackNet::pair();
        let a = GroupHub::new("lan", "host-a", net_a, MockAudio::new(), None);
        let b = GroupHub::new("lan", "host-b", net_b, MockAudio::new(), None);
        a.set_holding(Some(MAC.into())).await;
        b.pump_one().await.unwrap();
        let peers = b.peers().await;
        assert_eq!(peers.len(), 1);
        assert_eq!(peers[0].id, "host-a");
        assert_eq!(peers[0].holding.as_deref(), Some(MAC));
    }
}
