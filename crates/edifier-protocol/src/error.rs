use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ProtocolError {
    #[error("hex 长度必须是偶数")]
    OddHexLength,
    #[error("非法 hex 字符")]
    InvalidHex,
    #[error("未知机型档案")]
    UnknownProfile,
    #[error("载荷超过 255 字节")]
    PayloadTooLong,
    #[error("名称超过机型上限 {0} 字节")]
    NameTooLong(usize),
    #[error("环境声音量超出 -3..=3")]
    AmbientVolumeRange,
    #[error("意外的帧头 0x{0:02X}")]
    UnexpectedHead(u8),
    #[error("校验和错误: 期望 {expected:04X}, 实际 {actual:04X}")]
    Checksum { expected: u16, actual: u16 },
    #[error("设置文件不是 JSON 对象")]
    SettingsNotObject,
    #[error("设置文件缺少 commands 数组")]
    SettingsMissingCommands,
    #[error("不支持的设置文件版本 {0}")]
    SettingsVersion(u32),
    #[error("JSON 解析失败: {0}")]
    Json(String),
}
