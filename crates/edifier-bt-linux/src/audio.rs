use async_trait::async_trait;
use edifier_runtime::{AudioControl, AudioState, TransportError};
use tracing::warn;

mod bus;
mod observed;

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
    async fn audio_state(&self, address: &str) -> Result<AudioState, TransportError> {
        bus::audio_state(address).await
    }

    async fn connect_audio(&self, address: &str) -> Result<(), TransportError> {
        bus::set_connected(address, true).await
    }

    async fn disconnect_audio(&self, address: &str) -> Result<(), TransportError> {
        bus::set_connected(address, false).await
    }

    async fn suppress_autoreconnect(
        &self,
        address: &str,
        suppress: bool,
    ) -> Result<(), TransportError> {
        warn!(
            target: "edifier_bt_linux",
            address,
            suppress,
            "A2DP 抑制重连依赖 bluetoothd 策略, 无独立的公开接口"
        );
        Ok(())
    }
}
