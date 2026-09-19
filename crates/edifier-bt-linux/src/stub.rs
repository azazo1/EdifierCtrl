use async_trait::async_trait;
use edifier_runtime::{
    AudioControl, AudioState, HeadsetTransport, LinkKind, ScanResult, TransportError,
};

pub struct LinuxHeadset;

impl LinuxHeadset {
    pub fn new() -> Self {
        Self
    }
}

impl Default for LinuxHeadset {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl HeadsetTransport for LinuxHeadset {
    async fn scan(&self, _kind: LinkKind) -> Result<Vec<ScanResult>, TransportError> {
        Err(TransportError::Unsupported("需要 Linux BlueZ".into()))
    }
    async fn open(&self, _address: &str, _kind: LinkKind) -> Result<(), TransportError> {
        Err(TransportError::Unsupported("需要 Linux BlueZ".into()))
    }
    async fn write(&self, _bytes: &[u8]) -> Result<(), TransportError> {
        Err(TransportError::Unsupported("需要 Linux BlueZ".into()))
    }
    async fn recv(&self) -> Result<Vec<u8>, TransportError> {
        Err(TransportError::Unsupported("需要 Linux BlueZ".into()))
    }
    async fn close(&self) -> Result<(), TransportError> {
        Ok(())
    }
}

pub struct LinuxAudio;

impl LinuxAudio {
    pub fn new() -> Self {
        Self
    }
}

impl Default for LinuxAudio {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AudioControl for LinuxAudio {
    async fn audio_state(&self, _address: &str) -> Result<AudioState, TransportError> {
        Err(TransportError::Unsupported("需要 Linux BlueZ".into()))
    }
    async fn connect_audio(&self, _address: &str) -> Result<(), TransportError> {
        Err(TransportError::Unsupported("需要 Linux BlueZ".into()))
    }
    async fn disconnect_audio(&self, _address: &str) -> Result<(), TransportError> {
        Err(TransportError::Unsupported("需要 Linux BlueZ".into()))
    }
    async fn suppress_autoreconnect(
        &self,
        _address: &str,
        _suppress: bool,
    ) -> Result<(), TransportError> {
        Err(TransportError::Unsupported("需要 Linux BlueZ".into()))
    }
}
