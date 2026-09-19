use std::sync::Arc;

use async_trait::async_trait;
use edifier_protocol::Command;

use crate::host::HeadsetHost;
use crate::transport::{HeadsetTransport, TransportError};

/// 交接回退时发 CD (断开主机).
#[async_trait]
pub trait CdFallback: Send + Sync {
    async fn send_headset_disconnect(&self) -> Result<(), TransportError>;
}

#[async_trait]
impl<T: HeadsetTransport> CdFallback for HeadsetHost<T> {
    async fn send_headset_disconnect(&self) -> Result<(), TransportError> {
        self.send(&Command::DisconnectHost).await
    }
}

#[async_trait]
impl<T: CdFallback + ?Sized> CdFallback for Arc<T> {
    async fn send_headset_disconnect(&self) -> Result<(), TransportError> {
        (**self).send_headset_disconnect().await
    }
}

/// 计数用, 测试 CD 回退.
pub struct MockCd {
    pub hits: std::sync::atomic::AtomicU32,
}

impl MockCd {
    pub fn new() -> Self {
        Self {
            hits: std::sync::atomic::AtomicU32::new(0),
        }
    }

    pub fn count(&self) -> u32 {
        self.hits.load(std::sync::atomic::Ordering::SeqCst)
    }
}

impl Default for MockCd {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CdFallback for MockCd {
    async fn send_headset_disconnect(&self) -> Result<(), TransportError> {
        self.hits
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        tracing::info!(target: "edifier_runtime", "mock 发送 CD");
        Ok(())
    }
}
