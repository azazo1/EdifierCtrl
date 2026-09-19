//! Windows WinRT 蓝牙适配.

#[cfg(windows)]
mod addr;
#[cfg(windows)]
mod audio;
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
