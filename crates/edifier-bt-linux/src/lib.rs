//! Linux BlueZ 蓝牙适配. 非 Linux 上仅提供占位实现.

#[cfg(target_os = "linux")]
mod bluez;
#[cfg(target_os = "linux")]
pub use bluez::{LinuxAudio, LinuxHeadset};

#[cfg(not(target_os = "linux"))]
mod stub;
#[cfg(not(target_os = "linux"))]
pub use stub::{LinuxAudio, LinuxHeadset};
