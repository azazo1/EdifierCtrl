// FFI 单元测试只使用内存适配器, 不访问主机蓝牙或系统音频.
pub type PlatformHeadset = edifier_runtime::MockTransport;
pub type PlatformAudio = edifier_runtime::MockAudio;

pub fn new_headset() -> PlatformHeadset {
    PlatformHeadset::new(Vec::new())
}

pub fn new_audio() -> PlatformAudio {
    PlatformAudio::new()
}
