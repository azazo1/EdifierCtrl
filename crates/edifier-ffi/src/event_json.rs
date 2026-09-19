use edifier_group::{HandoffProgress, PeerInfo};
use edifier_runtime::{AudioState, LinkKind, RuntimeEvent, ScanResult};
use serde::Serialize;

use crate::notify_json::NotifyJson;

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EventJson {
    Empty,
    BtState {
        connected: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        link: Option<&'static str>,
        #[serde(skip_serializing_if = "Option::is_none")]
        address: Option<String>,
    },
    Headset {
        notification: NotifyJson,
    },
    Audio {
        state: &'static str,
    },
    Peer {
        peer: PeerInfo,
    },
    Handoff {
        progress: ProgressJson,
    },
    Message {
        text: String,
    },
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProgressJson {
    Requesting,
    WaitingPeer,
    Releasing,
    Connecting,
    FallbackCd,
    Done,
    Failed { reason: String },
    Busy,
}

#[derive(Serialize)]
pub struct ScanJson {
    pub address: String,
    pub name: String,
    pub kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_uuid: Option<String>,
}

pub fn link_str(kind: LinkKind) -> &'static str {
    match kind {
        LinkKind::Rfcomm => "rfcomm",
        LinkKind::Ble => "ble",
    }
}

pub fn event_json(ev: &RuntimeEvent) -> EventJson {
    match ev {
        RuntimeEvent::BtState {
            connected,
            kind,
            address,
        } => EventJson::BtState {
            connected: *connected,
            link: kind.map(link_str),
            address: address.clone(),
        },
        RuntimeEvent::Headset(n) => EventJson::Headset {
            notification: NotifyJson::from_notification(n),
        },
        RuntimeEvent::Audio(state) => EventJson::Audio {
            state: match state {
                AudioState::Unknown => "unknown",
                AudioState::Disconnected => "disconnected",
                AudioState::Connecting => "connecting",
                AudioState::Connected => "connected",
            },
        },
        RuntimeEvent::Peer(peer) => EventJson::Peer { peer: peer.clone() },
        RuntimeEvent::Handoff(p) => EventJson::Handoff {
            progress: progress_json(p),
        },
        RuntimeEvent::Message(text) => EventJson::Message { text: text.clone() },
    }
}

fn progress_json(p: &HandoffProgress) -> ProgressJson {
    match p {
        HandoffProgress::Requesting => ProgressJson::Requesting,
        HandoffProgress::WaitingPeer => ProgressJson::WaitingPeer,
        HandoffProgress::Releasing => ProgressJson::Releasing,
        HandoffProgress::Connecting => ProgressJson::Connecting,
        HandoffProgress::FallbackCd => ProgressJson::FallbackCd,
        HandoffProgress::Done => ProgressJson::Done,
        HandoffProgress::Failed(reason) => ProgressJson::Failed {
            reason: reason.clone(),
        },
        HandoffProgress::Busy => ProgressJson::Busy,
    }
}

pub fn scan_json(list: &[ScanResult]) -> Vec<ScanJson> {
    list.iter()
        .map(|s| ScanJson {
            address: s.address.clone(),
            name: s.name.clone(),
            kind: link_str(s.kind),
            service_uuid: s.service_uuid.clone(),
        })
        .collect()
}
