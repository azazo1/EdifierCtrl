use edifier_group::{Action, GroupMessage, HandoffProgress, MacAddr};
use serde::Serialize;

#[derive(Serialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum ActionJson {
    Send { message: GroupMessage },
    ConnectAudio { mac: String },
    DisconnectAudio { mac: String },
    SuppressAutoreconnect { mac: String, suppress: bool },
    SendHeadsetDisconnect,
    Report { progress: ProgressJson },
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

impl ActionJson {
    pub fn from_action(action: Action) -> Self {
        match action {
            Action::Send(message) => Self::Send { message },
            Action::ConnectAudio(mac) => Self::ConnectAudio {
                mac: mac.to_colon_string(),
            },
            Action::DisconnectAudio(mac) => Self::DisconnectAudio {
                mac: mac.to_colon_string(),
            },
            Action::SuppressAutoreconnect { mac, suppress } => Self::SuppressAutoreconnect {
                mac: mac.to_colon_string(),
                suppress,
            },
            Action::SendHeadsetDisconnect => Self::SendHeadsetDisconnect,
            Action::Report(progress) => Self::Report {
                progress: ProgressJson::from_progress(progress),
            },
        }
    }
}

impl ProgressJson {
    fn from_progress(p: HandoffProgress) -> Self {
        match p {
            HandoffProgress::Requesting => Self::Requesting,
            HandoffProgress::WaitingPeer => Self::WaitingPeer,
            HandoffProgress::Releasing => Self::Releasing,
            HandoffProgress::Connecting => Self::Connecting,
            HandoffProgress::FallbackCd => Self::FallbackCd,
            HandoffProgress::Done => Self::Done,
            HandoffProgress::Failed(reason) => Self::Failed { reason },
            HandoffProgress::Busy => Self::Busy,
        }
    }
}

pub fn actions_json(actions: Vec<Action>) -> Result<String, String> {
    let items: Vec<ActionJson> = actions.into_iter().map(ActionJson::from_action).collect();
    serde_json::to_string(&items).map_err(|e| e.to_string())
}

pub fn parse_mac(mac: &str) -> Result<MacAddr, String> {
    MacAddr::parse(mac).map_err(|e| e.to_string())
}
