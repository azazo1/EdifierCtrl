//! 漫步者耳机控制协议: 封包, 命令, 机型档案, 设置文件.

pub mod command;
pub mod constants;
pub mod error;
pub mod notification;
pub mod packet;
pub mod profile;
pub mod settings;

pub use command::{
    Command, ControlSettings, LdacMode, NoiseMode, Playback, SoundEffect,
};
pub use constants::*;
pub use error::ProtocolError;
pub use notification::{parse_notification, Notification};
pub use packet::{
    encode_tx, parse_hex, split_ble_chunks, to_hex, verify_checksum, FrameDecoder,
    IncomingFrame,
};
pub use profile::{DeviceProfile, Feature, ProfileId};
pub use settings::{SettingsCommand, SettingsFile, SETTINGS_FORMAT_VERSION};
