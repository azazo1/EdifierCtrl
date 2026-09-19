//! Windows WinRT 蓝牙适配.

#[cfg(windows)]
mod a2dp;
#[cfg(windows)]
mod addr;
#[cfg(windows)]
mod audio;
#[cfg(all(test, not(windows)))]
#[path = "audio/observed.rs"]
mod audio_observed;
#[cfg(all(test, not(windows)))]
#[path = "audio/services.rs"]
mod audio_services;
#[cfg(windows)]
mod ble;
#[cfg(windows)]
mod com;
#[cfg(windows)]
mod headset;
#[cfg(windows)]
mod io;
#[cfg(windows)]
mod rfcomm;

#[cfg(windows)]
pub use audio::WindowsAudio;
#[cfg(windows)]
pub use headset::WindowsHeadset;

#[cfg(not(windows))]
mod stub;
#[cfg(not(windows))]
pub use stub::{WindowsAudio, WindowsHeadset};
