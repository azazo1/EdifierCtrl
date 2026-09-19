use edifier_group::{HandoffProgress, PeerInfo};
use edifier_protocol::Notification;

use crate::transport::{AudioState, LinkKind};

#[derive(Debug, Clone)]
pub enum RuntimeEvent {
    BtState {
        connected: bool,
        kind: Option<LinkKind>,
        address: Option<String>,
    },
    Headset(Notification),
    Audio(AudioState),
    Peer(PeerInfo),
    Handoff(HandoffProgress),
    Message(String),
}
