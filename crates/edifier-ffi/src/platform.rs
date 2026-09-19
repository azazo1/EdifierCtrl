#[cfg(windows)]
pub type PlatformHeadset = edifier_bt_windows::WindowsHeadset;
#[cfg(windows)]
pub type PlatformAudio = edifier_bt_windows::WindowsAudio;

#[cfg(target_os = "linux")]
pub type PlatformHeadset = edifier_bt_linux::LinuxHeadset;
#[cfg(target_os = "linux")]
pub type PlatformAudio = edifier_bt_linux::LinuxAudio;

#[cfg(target_os = "android")]
pub type PlatformHeadset = edifier_bt_android::AndroidHeadset;
#[cfg(target_os = "android")]
pub type PlatformAudio = edifier_bt_android::AndroidAudio;

#[cfg(target_os = "macos")]
pub type PlatformHeadset = edifier_bt_macos::MacosHeadset;
#[cfg(target_os = "macos")]
pub type PlatformAudio = edifier_bt_macos::MacosAudio;

#[cfg(not(any(
    windows,
    target_os = "linux",
    target_os = "android",
    target_os = "macos"
)))]
pub type PlatformHeadset = edifier_runtime::MockTransport;
#[cfg(not(any(
    windows,
    target_os = "linux",
    target_os = "android",
    target_os = "macos"
)))]
pub type PlatformAudio = edifier_runtime::MockAudio;

pub fn new_headset() -> PlatformHeadset {
    #[cfg(windows)]
    {
        edifier_bt_windows::WindowsHeadset::new()
    }
    #[cfg(target_os = "linux")]
    {
        edifier_bt_linux::LinuxHeadset::new()
    }
    #[cfg(target_os = "android")]
    {
        edifier_bt_android::AndroidHeadset::new()
    }
    #[cfg(target_os = "macos")]
    {
        edifier_bt_macos::MacosHeadset::new()
    }
    #[cfg(not(any(
        windows,
        target_os = "linux",
        target_os = "android",
        target_os = "macos"
    )))]
    {
        edifier_runtime::MockTransport::new(Vec::new())
    }
}

pub fn new_audio() -> PlatformAudio {
    #[cfg(windows)]
    {
        edifier_bt_windows::WindowsAudio::new()
    }
    #[cfg(target_os = "linux")]
    {
        edifier_bt_linux::LinuxAudio::new()
    }
    #[cfg(target_os = "android")]
    {
        edifier_bt_android::AndroidAudio::new()
    }
    #[cfg(target_os = "macos")]
    {
        edifier_bt_macos::MacosAudio::new()
    }
    #[cfg(not(any(
        windows,
        target_os = "linux",
        target_os = "android",
        target_os = "macos"
    )))]
    {
        edifier_runtime::MockAudio::new()
    }
}
