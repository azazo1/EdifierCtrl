//! Android 蓝牙适配. 真机走 JNI `BluetoothBridge`, 其他目标仅占位.

#[cfg(target_os = "android")]
mod jni_bt;
#[cfg(target_os = "android")]
pub use jni_bt::{bind_jvm, AndroidAudio, AndroidHeadset};

#[cfg(not(target_os = "android"))]
mod stub;
#[cfg(not(target_os = "android"))]
pub use stub::{AndroidAudio, AndroidHeadset};
