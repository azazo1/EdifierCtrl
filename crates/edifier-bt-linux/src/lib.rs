//! Linux BlueZ 蓝牙适配. 非 Linux 上仅提供占位实现.

#[cfg(target_os = "linux")]
mod audio;
#[cfg(target_os = "linux")]
mod bluez;
#[cfg(target_os = "linux")]
pub use audio::LinuxAudio;
#[cfg(target_os = "linux")]
pub use bluez::LinuxHeadset;

#[cfg(all(test, not(target_os = "linux")))]
#[path = "audio/observed.rs"]
mod audio_observed;
#[cfg(all(test, not(target_os = "linux")))]
#[path = "bluez/link.rs"]
mod rfcomm_link;

#[cfg(not(target_os = "linux"))]
mod stub;
#[cfg(not(target_os = "linux"))]
pub use stub::{LinuxAudio, LinuxHeadset};
