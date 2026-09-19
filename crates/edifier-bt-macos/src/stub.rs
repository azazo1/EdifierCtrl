use async_trait::async_trait;
use edifier_runtime::{
    AudioControl, AudioState, HeadsetTransport, LinkKind, ScanResult, TransportError,
};

const MSG: &str = "macOS 蓝牙适配仅在 target_os=macos 上可用";

pub struct MacosHeadset;

impl MacosHeadset {
    pub fn new() -> Self {
        Self
    }
}

impl Default for MacosHeadset {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl HeadsetTransport for MacosHeadset {
    async fn scan(&self, _kind: LinkKind) -> Result<Vec<ScanResult>, TransportError> {
        Err(TransportError::Unsupported(MSG.into()))
    }
    async fn open(&self, _address: &str, _kind: LinkKind) -> Result<(), TransportError> {
        Err(TransportError::Unsupported(MSG.into()))
    }
    async fn write(&self, _bytes: &[u8]) -> Result<(), TransportError> {
        Err(TransportError::Unsupported(MSG.into()))
    }
    async fn recv(&self) -> Result<Vec<u8>, TransportError> {
        Err(TransportError::Unsupported(MSG.into()))
    }
    async fn close(&self) -> Result<(), TransportError> {
        Ok(())
    }
}

pub struct MacosAudio;

impl MacosAudio {
    pub fn new() -> Self {
        Self
    }
}

impl Default for MacosAudio {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AudioControl for MacosAudio {
    async fn audio_state(&self, _address: &str) -> Result<AudioState, TransportError> {
        Err(TransportError::Unsupported(MSG.into()))
    }
    async fn connect_audio(&self, _address: &str) -> Result<(), TransportError> {
        Err(TransportError::Unsupported(MSG.into()))
    }
    async fn disconnect_audio(&self, _address: &str) -> Result<(), TransportError> {
        Err(TransportError::Unsupported(MSG.into()))
    }
    async fn suppress_autoreconnect(
        &self,
        _address: &str,
        _suppress: bool,
    ) -> Result<(), TransportError> {
        Err(TransportError::Unsupported(MSG.into()))
    }
}
