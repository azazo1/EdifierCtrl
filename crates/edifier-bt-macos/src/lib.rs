//! macOS 蓝牙适配. 真机走 Swift IOBluetooth 桥, 其他目标仅占位.

#[cfg(target_os = "macos")]
mod iobluetooth;
#[cfg(target_os = "macos")]
pub use iobluetooth::{MacosAudio, MacosHeadset};

#[cfg(not(target_os = "macos"))]
mod stub;
#[cfg(not(target_os = "macos"))]
pub use stub::{MacosAudio, MacosHeadset};
