use crate::key::{sign, verify, GroupId, GroupKey};
use crate::message::{Envelope, ENVELOPE_VERSION, MAX_SKEW_MS};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnvelopeError {
    Version,
    GroupId,
    Timestamp,
    Mac,
    Json(String),
}

pub fn seal(
    key: &GroupKey,
    group_id: GroupId,
    ts_ms: u64,
    body: crate::message::GroupMessage,
) -> Result<Envelope, EnvelopeError> {
    let mut env = Envelope {
        version: ENVELOPE_VERSION,
        group_id: group_id.to_hex(),
        ts_ms,
        body,
        mac: String::new(),
    };
    let canonical = env
        .canonical_without_mac()
        .map_err(|e| EnvelopeError::Json(e.to_string()))?;
    env.mac = hex::encode(sign(key, &canonical));
    Ok(env)
}

pub fn open(
    key: &GroupKey,
    group_id: GroupId,
    now_ms: u64,
    env: &Envelope,
) -> Result<crate::message::GroupMessage, EnvelopeError> {
    if env.version != ENVELOPE_VERSION {
        return Err(EnvelopeError::Version);
    }
    if env.group_id != group_id.to_hex() {
        return Err(EnvelopeError::GroupId);
    }
    let skew = now_ms.abs_diff(env.ts_ms);
    if skew > MAX_SKEW_MS {
        return Err(EnvelopeError::Timestamp);
    }
    let mac_bytes = hex::decode(&env.mac).map_err(|_| EnvelopeError::Mac)?;
    let mut mac = [0u8; 32];
    if mac_bytes.len() != 32 {
        return Err(EnvelopeError::Mac);
    }
    mac.copy_from_slice(&mac_bytes);
    let canonical = env
        .canonical_without_mac()
        .map_err(|e| EnvelopeError::Json(e.to_string()))?;
    if !verify(key, &canonical, &mac) {
        return Err(EnvelopeError::Mac);
    }
    Ok(env.body.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::key::derive_group;
    use crate::message::{GroupMessage, PeerInfo};

    #[test]
    fn seal_open_roundtrip() {
        let (gid, key) = derive_group("lan");
        let body = GroupMessage::Announce {
            peer: PeerInfo {
                id: "a".into(),
                hostname: "pc".into(),
                os: "windows".into(),
                can_audio: true,
                holding: None,
                app_version: "0.1.0".into(),
            },
        };
        let env = seal(&key, gid, 1000, body.clone()).unwrap();
        assert_eq!(open(&key, gid, 1000, &env).unwrap(), body);
    }

    #[test]
    fn reject_wrong_group() {
        let (gid, key) = derive_group("lan");
        let (other, _) = derive_group("nope");
        let env = seal(
            &key,
            gid,
            1000,
            GroupMessage::HandoffBusy {
                nonce: "00".repeat(16),
            },
        )
        .unwrap();
        assert_eq!(open(&key, other, 1000, &env), Err(EnvelopeError::GroupId));
    }
}
