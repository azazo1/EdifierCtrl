use edifier_group::MacAddr;
use edifier_runtime::TransportError;

pub fn parse_u64(s: &str) -> Result<u64, TransportError> {
    let mac = MacAddr::parse(s).map_err(|e| TransportError::NotFound(e.to_string()))?;
    let b = mac.0;
    Ok(((b[0] as u64) << 40)
        | ((b[1] as u64) << 32)
        | ((b[2] as u64) << 24)
        | ((b[3] as u64) << 16)
        | ((b[4] as u64) << 8)
        | (b[5] as u64))
}

pub fn from_u64(addr: u64) -> String {
    MacAddr([
        ((addr >> 40) & 0xff) as u8,
        ((addr >> 32) & 0xff) as u8,
        ((addr >> 24) & 0xff) as u8,
        ((addr >> 16) & 0xff) as u8,
        ((addr >> 8) & 0xff) as u8,
        (addr & 0xff) as u8,
    ])
    .to_colon_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let s = "11:22:33:44:55:66";
        assert_eq!(from_u64(parse_u64(s).unwrap()), s);
    }
}
