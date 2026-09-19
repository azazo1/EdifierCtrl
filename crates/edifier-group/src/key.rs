use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GroupId(pub [u8; 16]);

#[derive(Clone)]
pub struct GroupKey(pub [u8; 32]);

impl GroupId {
    pub fn to_hex(self) -> String {
        hex::encode(self.0)
    }
}

/// 口令派生组 ID 和 HMAC 密钥, 不保存明文口令.
pub fn derive_group(passphrase: &str) -> (GroupId, GroupKey) {
    let mut id_hash = Sha256::new();
    id_hash.update(b"edifierctrl-group-v1\0");
    id_hash.update(passphrase.as_bytes());
    let id_bytes = id_hash.finalize();
    let mut gid = [0u8; 16];
    gid.copy_from_slice(&id_bytes[..16]);

    let mut key_hash = Sha256::new();
    key_hash.update(b"edifierctrl-group-key-v1\0");
    key_hash.update(passphrase.as_bytes());
    let key_bytes = key_hash.finalize();
    let mut key = [0u8; 32];
    key.copy_from_slice(&key_bytes);

    (GroupId(gid), GroupKey(key))
}

pub fn sign(key: &GroupKey, body: &[u8]) -> [u8; 32] {
    let mut mac = HmacSha256::new_from_slice(&key.0).expect("HMAC-SHA256 接受 32 字节密钥");
    mac.update(body);
    let result = mac.finalize().into_bytes();
    let mut out = [0u8; 32];
    out.copy_from_slice(&result);
    out
}

pub fn verify(key: &GroupKey, body: &[u8], mac: &[u8; 32]) -> bool {
    sign(key, body) == *mac
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_passphrase_same_id() {
        let (a, _) = derive_group("demo");
        let (b, _) = derive_group("demo");
        assert_eq!(a, b);
        let (c, _) = derive_group("other");
        assert_ne!(a, c);
    }

    #[test]
    fn hmac_roundtrip() {
        let (_, key) = derive_group("demo");
        let mac = sign(&key, b"hello");
        assert!(verify(&key, b"hello", &mac));
        assert!(!verify(&key, b"hallo", &mac));
    }
}
