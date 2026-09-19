use serde::{Deserialize, Serialize};

use crate::mac::MacAddr;

pub const ENVELOPE_VERSION: u8 = 1;
pub const MAX_SKEW_MS: u64 = 30_000;

pub type Nonce = [u8; 16];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerInfo {
    pub id: String,
    pub hostname: String,
    pub os: String,
    pub can_audio: bool,
    pub holding: Option<String>,
    pub app_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GroupMessage {
    Announce {
        peer: PeerInfo,
    },
    HandoffRequest {
        headphone: String,
        nonce: String,
        deadline_ms: u64,
    },
    HandoffHasAudio {
        nonce: String,
    },
    HandoffNoAudio {
        nonce: String,
    },
    HandoffReleased {
        nonce: String,
    },
    HandoffTaken {
        nonce: String,
    },
    HandoffAbort {
        nonce: String,
        reason: String,
    },
    HandoffBusy {
        nonce: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Envelope {
    pub version: u8,
    pub group_id: String,
    pub ts_ms: u64,
    pub body: GroupMessage,
    #[serde(default)]
    pub mac: String,
}

impl Envelope {
    pub fn canonical_without_mac(&self) -> Result<Vec<u8>, serde_json::Error> {
        let mut clone = self.clone();
        clone.mac.clear();
        serde_json::to_vec(&clone)
    }
}

pub fn nonce_to_hex(nonce: Nonce) -> String {
    hex::encode(nonce)
}

pub fn parse_nonce(s: &str) -> Option<Nonce> {
    let bytes = hex::decode(s).ok()?;
    if bytes.len() != 16 {
        return None;
    }
    let mut n = [0u8; 16];
    n.copy_from_slice(&bytes);
    Some(n)
}

pub fn parse_headphone(s: &str) -> Option<MacAddr> {
    MacAddr::parse(s).ok()
}
