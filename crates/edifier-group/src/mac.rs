use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MacAddr(pub [u8; 6]);

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum MacParseError {
    #[error("MAC 地址格式无效")]
    Invalid,
}

impl MacAddr {
    pub fn parse(input: &str) -> Result<Self, MacParseError> {
        let hex: String = input
            .chars()
            .filter(|c| *c != ':' && *c != '-' && !c.is_whitespace())
            .collect();
        if hex.len() != 12 {
            return Err(MacParseError::Invalid);
        }
        let bytes = hex::decode(hex).map_err(|_| MacParseError::Invalid)?;
        let mut mac = [0u8; 6];
        mac.copy_from_slice(&bytes);
        Ok(Self(mac))
    }

    pub fn to_colon_string(self) -> String {
        let b = self.0;
        format!(
            "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
            b[0], b[1], b[2], b[3], b[4], b[5]
        )
    }
}
