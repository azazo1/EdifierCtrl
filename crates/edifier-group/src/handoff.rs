use edifier_protocol::AUDIO_CONNECT_GAP_MS;
use tracing::info;

use crate::mac::MacAddr;
use crate::message::{nonce_to_hex, GroupMessage, Nonce};

pub const HANDOFF_DEADLINE_MS: u64 = 6000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Idle,
    WaitingRelease {
        nonce: Nonce,
        headphone: MacAddr,
        deadline_ms: u64,
    },
    WaitingFallbackGap {
        nonce: Nonce,
        headphone: MacAddr,
        ready_ms: u64,
    },
    ConnectingAudio {
        nonce: Nonce,
        headphone: MacAddr,
    },
    Releasing {
        nonce: Nonce,
        headphone: MacAddr,
        deadline_ms: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HandoffProgress {
    Requesting,
    WaitingPeer,
    Releasing,
    Connecting,
    FallbackCd,
    Done,
    Failed(String),
    Busy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Send(GroupMessage),
    ConnectAudio(MacAddr),
    DisconnectAudio(MacAddr),
    SuppressAutoreconnect { mac: MacAddr, suppress: bool },
    SendHeadsetDisconnect,
    Report(HandoffProgress),
}

#[derive(Debug, Clone)]
pub struct HandoffMachine {
    pub local_id: String,
    pub has_audio: bool,
    pub can_control_headset: bool,
    phase: Phase,
    peer_has_audio: bool,
    peer_has_no_audio: bool,
}

impl HandoffMachine {
    pub fn new(local_id: impl Into<String>) -> Self {
        Self {
            local_id: local_id.into(),
            has_audio: false,
            can_control_headset: false,
            phase: Phase::Idle,
            peer_has_audio: false,
            peer_has_no_audio: false,
        }
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    pub fn claim(&mut self, headphone: MacAddr, nonce: Nonce, now_ms: u64) -> Vec<Action> {
        if self.phase != Phase::Idle {
            return vec![Action::Report(HandoffProgress::Busy)];
        }
        self.peer_has_audio = false;
        self.peer_has_no_audio = false;
        let deadline_ms = now_ms.saturating_add(HANDOFF_DEADLINE_MS);
        self.phase = Phase::WaitingRelease {
            nonce,
            headphone,
            deadline_ms,
        };
        info!(target: "edifier_group::handoff", headphone = %headphone.to_colon_string(), "发起交接请求");
        vec![
            Action::Report(HandoffProgress::Requesting),
            Action::Send(GroupMessage::HandoffRequest {
                headphone: headphone.to_colon_string(),
                nonce: nonce_to_hex(nonce),
                deadline_ms,
            }),
        ]
    }

    pub fn on_message(&mut self, msg: &GroupMessage, now_ms: u64) -> Vec<Action> {
        match msg {
            GroupMessage::HandoffRequest {
                headphone,
                nonce,
                deadline_ms,
            } => self.on_request(headphone, nonce, *deadline_ms, now_ms),
            GroupMessage::HandoffHasAudio { nonce } => self.on_peer_has_audio(nonce),
            GroupMessage::HandoffReleased { nonce } => self.on_released(nonce),
            GroupMessage::HandoffTaken { nonce } => self.on_taken(nonce),
            GroupMessage::HandoffAbort { nonce, reason } => self.on_abort(nonce, reason),
            GroupMessage::HandoffBusy { nonce } => self.on_busy(nonce),
            GroupMessage::HandoffNoAudio { nonce } => self.on_no_audio(nonce),
            GroupMessage::Announce { .. } => Vec::new(),
        }
    }

    pub fn on_audio_disconnected(&mut self) -> Vec<Action> {
        match self.phase {
            Phase::Releasing { nonce, .. } => {
                self.has_audio = false;
                info!(target: "edifier_group::handoff", "本机音频已释放");
                vec![Action::Send(GroupMessage::HandoffReleased {
                    nonce: nonce_to_hex(nonce),
                })]
            }
            _ => Vec::new(),
        }
    }

    pub fn on_audio_connected(&mut self) -> Vec<Action> {
        match self.phase {
            Phase::ConnectingAudio { nonce, headphone } => {
                self.phase = Phase::Idle;
                self.has_audio = true;
                info!(target: "edifier_group::handoff", mac = %headphone.to_colon_string(), "本机已接管音频");
                vec![
                    Action::Send(GroupMessage::HandoffTaken {
                        nonce: nonce_to_hex(nonce),
                    }),
                    Action::Report(HandoffProgress::Done),
                ]
            }
            _ => Vec::new(),
        }
    }

    pub fn on_audio_failed(&mut self, reason: &str) -> Vec<Action> {
        match self.phase {
            Phase::ConnectingAudio { nonce, headphone }
            | Phase::Releasing { nonce, headphone, .. }
            | Phase::WaitingFallbackGap { nonce, headphone, .. }
            | Phase::WaitingRelease { nonce, headphone, .. } => {
                self.phase = Phase::Idle;
                vec![
                    Action::Send(GroupMessage::HandoffAbort {
                        nonce: nonce_to_hex(nonce),
                        reason: reason.to_string(),
                    }),
                    Action::SuppressAutoreconnect {
                        mac: headphone,
                        suppress: false,
                    },
                    Action::Report(HandoffProgress::Failed(reason.to_string())),
                ]
            }
            _ => Vec::new(),
        }
    }

    pub fn tick(&mut self, now_ms: u64) -> Vec<Action> {
        match self.phase {
            Phase::WaitingRelease {
                nonce,
                headphone,
                deadline_ms,
            } if now_ms >= deadline_ms => {
                if self.peer_has_no_audio && !self.peer_has_audio {
                    self.fallback_or_connect(nonce, headphone)
                } else {
                    self.fallback_or_fail(nonce, headphone, now_ms)
                }
            }
            Phase::WaitingFallbackGap {
                nonce,
                headphone,
                ready_ms,
            } if now_ms >= ready_ms => {
                self.phase = Phase::ConnectingAudio { nonce, headphone };
                vec![
                    Action::Report(HandoffProgress::Connecting),
                    Action::ConnectAudio(headphone),
                ]
            }
            Phase::Releasing {
                deadline_ms,
                ..
            } if now_ms >= deadline_ms => self.on_audio_failed("释放音频超时"),
            _ => Vec::new(),
        }
    }

    fn on_request(
        &mut self,
        headphone: &str,
        nonce_hex: &str,
        deadline_ms: u64,
        now_ms: u64,
    ) -> Vec<Action> {
        let Some(mac) = MacAddr::parse(headphone).ok() else {
            return Vec::new();
        };
        let Some(nonce) = crate::message::parse_nonce(nonce_hex) else {
            return Vec::new();
        };
        if deadline_ms <= now_ms {
            return Vec::new();
        }
        if self.phase != Phase::Idle {
            return vec![Action::Send(GroupMessage::HandoffBusy {
                nonce: nonce_hex.to_string(),
            })];
        }
        if !self.has_audio {
            return vec![Action::Send(GroupMessage::HandoffNoAudio {
                nonce: nonce_hex.to_string(),
            })];
        }
        self.phase = Phase::Releasing {
            nonce,
            headphone: mac,
            deadline_ms: deadline_ms.min(now_ms.saturating_add(HANDOFF_DEADLINE_MS)),
        };
        info!(target: "edifier_group::handoff", "本机持有音频, 开始释放");
        vec![
            Action::Send(GroupMessage::HandoffHasAudio {
                nonce: nonce_hex.to_string(),
            }),
            Action::Report(HandoffProgress::Releasing),
            Action::SuppressAutoreconnect {
                mac,
                suppress: true,
            },
            Action::DisconnectAudio(mac),
        ]
    }

    fn on_peer_has_audio(&mut self, nonce_hex: &str) -> Vec<Action> {
        if !self.nonce_matches(nonce_hex) {
            return Vec::new();
        }
        if matches!(self.phase, Phase::WaitingRelease { .. }) {
            self.peer_has_audio = true;
            vec![Action::Report(HandoffProgress::WaitingPeer)]
        } else {
            Vec::new()
        }
    }

    fn on_no_audio(&mut self, nonce_hex: &str) -> Vec<Action> {
        if !self.nonce_matches(nonce_hex) {
            return Vec::new();
        }
        if matches!(self.phase, Phase::WaitingRelease { .. }) {
            // 广播组中一个成员无音频, 不能代表其他成员已经释放.
            self.peer_has_no_audio = true;
        }
        Vec::new()
    }

    fn on_released(&mut self, nonce_hex: &str) -> Vec<Action> {
        if !self.nonce_matches(nonce_hex) {
            return Vec::new();
        }
        match self.phase {
            Phase::WaitingRelease { nonce, headphone, .. } => {
                self.phase = Phase::ConnectingAudio { nonce, headphone };
                vec![
                    Action::Report(HandoffProgress::Connecting),
                    Action::ConnectAudio(headphone),
                ]
            }
            _ => Vec::new(),
        }
    }

    fn on_taken(&mut self, nonce_hex: &str) -> Vec<Action> {
        if !self.nonce_matches(nonce_hex) {
            return Vec::new();
        }
        if matches!(self.phase, Phase::Releasing { .. }) {
            self.phase = Phase::Idle;
            self.has_audio = false;
            vec![Action::Report(HandoffProgress::Done)]
        } else {
            Vec::new()
        }
    }

    fn on_abort(&mut self, nonce_hex: &str, reason: &str) -> Vec<Action> {
        if !self.nonce_matches(nonce_hex) {
            return Vec::new();
        }
        let mac = match self.phase {
            Phase::Releasing { headphone, .. }
            | Phase::WaitingRelease { headphone, .. }
            | Phase::ConnectingAudio { headphone, .. }
            | Phase::WaitingFallbackGap { headphone, .. } => Some(headphone),
            Phase::Idle => None,
        };
        self.phase = Phase::Idle;
        let mut actions = vec![Action::Report(HandoffProgress::Failed(reason.to_string()))];
        if let Some(mac) = mac {
            actions.push(Action::SuppressAutoreconnect {
                mac,
                suppress: false,
            });
        }
        actions
    }

    fn on_busy(&mut self, nonce_hex: &str) -> Vec<Action> {
        if !self.nonce_matches(nonce_hex) {
            return Vec::new();
        }
        if !matches!(self.phase, Phase::WaitingRelease { .. }) {
            return Vec::new();
        }
        self.phase = Phase::Idle;
        vec![Action::Report(HandoffProgress::Busy)]
    }

    fn fallback_or_fail(&mut self, nonce: Nonce, headphone: MacAddr, now_ms: u64) -> Vec<Action> {
        if self.can_control_headset {
            self.phase = Phase::WaitingFallbackGap {
                nonce,
                headphone,
                ready_ms: now_ms.saturating_add(AUDIO_CONNECT_GAP_MS),
            };
            info!(target: "edifier_group::handoff", "对端超时, 回退发送 CD");
            vec![
                Action::Report(HandoffProgress::FallbackCd),
                Action::SendHeadsetDisconnect,
            ]
        } else {
            self.phase = Phase::Idle;
            vec![Action::Report(HandoffProgress::Failed(
                "对端未释放且本机无法控制耳机".into(),
            ))]
        }
    }

    fn fallback_or_connect(&mut self, nonce: Nonce, headphone: MacAddr) -> Vec<Action> {
        self.phase = Phase::ConnectingAudio { nonce, headphone };
        vec![
            Action::Report(HandoffProgress::Connecting),
            Action::ConnectAudio(headphone),
        ]
    }

    fn nonce_matches(&self, nonce_hex: &str) -> bool {
        let current = match self.phase {
            Phase::Idle => return false,
            Phase::WaitingRelease { nonce, .. }
            | Phase::WaitingFallbackGap { nonce, .. }
            | Phase::ConnectingAudio { nonce, .. }
            | Phase::Releasing { nonce, .. } => nonce,
        };
        crate::message::parse_nonce(nonce_hex)
            .map(|n| n == current)
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mac() -> MacAddr {
        MacAddr::parse("11:22:33:44:55:66").unwrap()
    }

    fn nonce() -> Nonce {
        [7u8; 16]
    }

    fn nonce_hex() -> String {
        nonce_to_hex(nonce())
    }

    #[test]
    fn happy_path_holder_releases() {
        let mut requester = HandoffMachine::new("b");
        let mut holder = HandoffMachine::new("a");
        holder.has_audio = true;

        let req_actions = requester.claim(mac(), nonce(), 0);
        let request = req_actions
            .iter()
            .find_map(|a| match a {
                Action::Send(m) => Some(m.clone()),
                _ => None,
            })
            .unwrap();

        let holder_actions = holder.on_message(&request, 10);
        assert!(holder_actions
            .iter()
            .any(|a| matches!(a, Action::DisconnectAudio(_))));
        assert!(matches!(holder.phase(), Phase::Releasing { .. }));

        let released = holder
            .on_audio_disconnected()
            .into_iter()
            .find_map(|a| match a {
                Action::Send(m) => Some(m),
                _ => None,
            })
            .unwrap();

        let take = requester.on_message(&released, 20);
        assert!(take
            .iter()
            .any(|a| matches!(a, Action::ConnectAudio(_))));

        let done = requester.on_audio_connected();
        assert!(done
            .iter()
            .any(|a| matches!(a, Action::Report(HandoffProgress::Done))));
        assert!(requester.has_audio);

        let holder_done = holder.on_message(
            &GroupMessage::HandoffTaken {
                nonce: nonce_hex(),
            },
            30,
        );
        assert!(holder_done
            .iter()
            .any(|a| matches!(a, Action::Report(HandoffProgress::Done))));
        assert!(!holder.has_audio);
        assert_eq!(holder.phase(), Phase::Idle);
    }

    #[test]
    fn busy_when_holder_not_idle() {
        let mut holder = HandoffMachine::new("a");
        holder.has_audio = true;
        holder.claim(mac(), nonce(), 0);
        let msg = GroupMessage::HandoffRequest {
            headphone: mac().to_colon_string(),
            nonce: hex::encode([8u8; 16]),
            deadline_ms: 2000,
        };
        let actions = holder.on_message(&msg, 1);
        assert!(actions
            .iter()
            .any(|a| matches!(a, Action::Send(GroupMessage::HandoffBusy { .. }))));
    }

    #[test]
    fn timeout_without_control_fails() {
        let mut requester = HandoffMachine::new("b");
        requester.claim(mac(), nonce(), 0);
        let actions = requester.tick(HANDOFF_DEADLINE_MS);
        assert!(actions.iter().any(|a| matches!(
            a,
            Action::Report(HandoffProgress::Failed(_))
        )));
        assert_eq!(requester.phase(), Phase::Idle);
    }

    #[test]
    fn timeout_with_ble_sends_cd() {
        let mut requester = HandoffMachine::new("b");
        requester.can_control_headset = true;
        requester.claim(mac(), nonce(), 0);
        let actions = requester.tick(HANDOFF_DEADLINE_MS);
        assert!(actions
            .iter()
            .any(|a| matches!(a, Action::SendHeadsetDisconnect)));
        assert!(matches!(
            requester.phase(),
            Phase::WaitingFallbackGap { .. }
        ));
        let later = requester.tick(HANDOFF_DEADLINE_MS + AUDIO_CONNECT_GAP_MS);
        assert!(later
            .iter()
            .any(|a| matches!(a, Action::ConnectAudio(_))));
    }

    #[test]
    fn no_audio_waits_for_other_peers_until_deadline() {
        let mut requester = HandoffMachine::new("b");
        requester.claim(mac(), nonce(), 0);
        let actions = requester.on_message(
            &GroupMessage::HandoffNoAudio { nonce: nonce_hex() }, 5,
        );
        assert!(actions.is_empty());
        assert!(requester.tick(HANDOFF_DEADLINE_MS - 1).is_empty());
        assert!(requester.tick(HANDOFF_DEADLINE_MS).iter()
            .any(|a| matches!(a, Action::ConnectAudio(_))));
    }

    #[test]
    fn bystander_no_audio_does_not_override_holder() {
        for bystander_first in [false, true] {
            let mut requester = HandoffMachine::new("b");
            requester.claim(mac(), nonce(), 0);
            let holder = GroupMessage::HandoffHasAudio { nonce: nonce_hex() };
            let bystander = GroupMessage::HandoffNoAudio { nonce: nonce_hex() };
            let replies = if bystander_first { [bystander, holder] } else { [holder, bystander] };
            for reply in replies {
                assert!(!requester.on_message(&reply, 5).iter()
                    .any(|a| matches!(a, Action::ConnectAudio(_))));
            }
            assert!(!requester.tick(HANDOFF_DEADLINE_MS).iter()
                .any(|a| matches!(a, Action::ConnectAudio(_))));
            assert_eq!(requester.phase(), Phase::Idle);
        }
    }

    #[test]
    fn releasing_failure_and_timeout_abort_and_restore() {
        for timeout in [false, true] {
            let mut holder = HandoffMachine::new("a");
            holder.has_audio = true;
            holder.on_message(&GroupMessage::HandoffRequest {
                headphone: mac().to_colon_string(), nonce: nonce_hex(), deadline_ms: 100,
            }, 0);
            let actions = if timeout { holder.tick(100) } else { holder.on_audio_failed("断开失败") };
            assert_eq!(holder.phase(), Phase::Idle);
            assert!(holder.has_audio);
            assert!(actions.iter().any(|a| matches!(a, Action::Send(GroupMessage::HandoffAbort { .. }))));
            assert!(actions.iter().any(|a| matches!(a, Action::SuppressAutoreconnect { suppress: false, .. })));
            assert!(!actions.iter().any(|a| matches!(a, Action::Send(GroupMessage::HandoffReleased { .. }))));
        }
    }

    #[test]
    fn remote_deadline_is_capped_and_expired_requests_are_ignored() {
        let mut holder = HandoffMachine::new("a");
        holder.has_audio = true;
        let mut req = GroupMessage::HandoffRequest {
            headphone: mac().to_colon_string(), nonce: nonce_hex(), deadline_ms: 10,
        };
        assert!(holder.on_message(&req, 10).is_empty());
        assert_eq!(holder.phase(), Phase::Idle);
        if let GroupMessage::HandoffRequest { deadline_ms, .. } = &mut req {
            *deadline_ms = u64::MAX;
        }
        holder.on_message(&req, 10);
        assert!(matches!(holder.phase(), Phase::Releasing { deadline_ms, .. } if deadline_ms == 10 + HANDOFF_DEADLINE_MS));
    }
}
