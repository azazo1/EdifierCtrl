use async_trait::async_trait;
use edifier_runtime::{
    AudioControl, AudioState, HeadsetTransport, LinkKind, ScanResult, TransportError,
};

const MSG: &str = "Android 蓝牙适配仅在 target_os=android 上可用";

pub struct AndroidHeadset;

impl AndroidHeadset {
    pub fn new() -> Self {
        Self
    }
}

impl Default for AndroidHeadset {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl HeadsetTransport for AndroidHeadset {
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

pub struct AndroidAudio;

impl AndroidAudio {
    pub fn new() -> Self {
        Self
    }
}

impl Default for AndroidAudio {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AudioControl for AndroidAudio {
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
