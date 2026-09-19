use std::sync::Arc;

use async_trait::async_trait;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkKind {
    Rfcomm,
    Ble,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanResult {
    pub address: String,
    pub name: String,
    pub kind: LinkKind,
    pub service_uuid: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioState {
    Unknown,
    Disconnected,
    Connecting,
    Connected,
}

#[derive(Debug, Error)]
pub enum TransportError {
    #[error("蓝牙不可用: {0}")]
    Unavailable(String),
    #[error("设备未找到: {0}")]
    NotFound(String),
    #[error("连接失败: {0}")]
    Connect(String),
    #[error("写入失败: {0}")]
    Write(String),
    #[error("权限不足: {0}")]
    Permission(String),
    #[error("平台不支持: {0}")]
    Unsupported(String),
    #[error("连接已关闭")]
    Closed,
}

#[async_trait]
pub trait HeadsetTransport: Send + Sync {
    async fn scan(&self, kind: LinkKind) -> Result<Vec<ScanResult>, TransportError>;
    async fn open(&self, address: &str, kind: LinkKind) -> Result<(), TransportError>;
    async fn write(&self, bytes: &[u8]) -> Result<(), TransportError>;
    /// 阻塞直到有数据. 对端或本地关闭时返回 Closed.
    async fn recv(&self) -> Result<Vec<u8>, TransportError>;
    async fn close(&self) -> Result<(), TransportError>;
}

#[async_trait]
pub trait AudioControl: Send + Sync {
    async fn audio_state(&self, address: &str) -> Result<AudioState, TransportError>;
    async fn connect_audio(&self, address: &str) -> Result<(), TransportError>;
    async fn disconnect_audio(&self, address: &str) -> Result<(), TransportError>;
    async fn suppress_autoreconnect(
        &self,
        address: &str,
        suppress: bool,
    ) -> Result<(), TransportError>;
}

#[async_trait]
impl<T: AudioControl + ?Sized> AudioControl for Arc<T> {
    async fn audio_state(&self, address: &str) -> Result<AudioState, TransportError> {
        (**self).audio_state(address).await
    }
    async fn connect_audio(&self, address: &str) -> Result<(), TransportError> {
        (**self).connect_audio(address).await
    }
    async fn disconnect_audio(&self, address: &str) -> Result<(), TransportError> {
        (**self).disconnect_audio(address).await
    }
    async fn suppress_autoreconnect(
        &self,
        address: &str,
        suppress: bool,
    ) -> Result<(), TransportError> {
        (**self).suppress_autoreconnect(address, suppress).await
    }
}
