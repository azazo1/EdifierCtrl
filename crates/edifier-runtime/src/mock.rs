use std::sync::Mutex as StdMutex;

use tokio::sync::{mpsc, Mutex};

use crate::transport::{
    AudioControl, AudioState, HeadsetTransport, LinkKind, ScanResult, TransportError,
};

/// 内存传输, 供 runtime 测试和 UI 联调.
pub struct MockTransport {
    scan: Vec<ScanResult>,
    written: Mutex<Vec<Vec<u8>>>,
    rx: Mutex<mpsc::UnboundedReceiver<Vec<u8>>>,
    tx: StdMutex<Option<mpsc::UnboundedSender<Vec<u8>>>>,
    opened: Mutex<Option<(String, LinkKind)>>,
}

impl MockTransport {
    pub fn new(scan: Vec<ScanResult>) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        Self {
            scan,
            written: Mutex::new(Vec::new()),
            rx: Mutex::new(rx),
            tx: StdMutex::new(Some(tx)),
            opened: Mutex::new(None),
        }
    }

    pub fn inject(&self, bytes: Vec<u8>) {
        if let Some(tx) = self.tx.lock().expect("mock tx").as_ref() {
            let _ = tx.send(bytes);
        }
    }

    pub fn close_incoming(&self) {
        *self.tx.lock().expect("mock tx") = None;
    }

    pub async fn written(&self) -> Vec<Vec<u8>> {
        self.written.lock().await.clone()
    }
}

#[async_trait::async_trait]
impl HeadsetTransport for MockTransport {
    async fn scan(&self, kind: LinkKind) -> Result<Vec<ScanResult>, TransportError> {
        Ok(self
            .scan
            .iter()
            .filter(|s| s.kind == kind)
            .cloned()
            .collect())
    }

    async fn open(&self, address: &str, kind: LinkKind) -> Result<(), TransportError> {
        *self.opened.lock().await = Some((address.to_string(), kind));
        Ok(())
    }

    async fn write(&self, bytes: &[u8]) -> Result<(), TransportError> {
        if self.opened.lock().await.is_none() {
            return Err(TransportError::Connect("尚未连接".into()));
        }
        self.written.lock().await.push(bytes.to_vec());
        Ok(())
    }

    async fn recv(&self) -> Result<Vec<u8>, TransportError> {
        let mut rx = self.rx.lock().await;
        rx.recv().await.ok_or(TransportError::Closed)
    }

    async fn close(&self) -> Result<(), TransportError> {
        *self.opened.lock().await = None;
        *self.tx.lock().expect("mock tx") = None;
        Ok(())
    }
}

/// 音频控制的空实现, 状态记在内存里.
pub struct MockAudio {
    state: Mutex<AudioState>,
}

impl MockAudio {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(AudioState::Disconnected),
        }
    }
}

impl Default for MockAudio {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl AudioControl for MockAudio {
    async fn audio_state(&self, _address: &str) -> Result<AudioState, TransportError> {
        Ok(*self.state.lock().await)
    }

    async fn connect_audio(&self, address: &str) -> Result<(), TransportError> {
        tracing::info!(target: "edifier_runtime", address, "mock 连接音频");
        *self.state.lock().await = AudioState::Connected;
        Ok(())
    }

    async fn disconnect_audio(&self, address: &str) -> Result<(), TransportError> {
        tracing::info!(target: "edifier_runtime", address, "mock 断开音频");
        *self.state.lock().await = AudioState::Disconnected;
        Ok(())
    }

    async fn suppress_autoreconnect(
        &self,
        address: &str,
        suppress: bool,
    ) -> Result<(), TransportError> {
        tracing::info!(target: "edifier_runtime", address, suppress, "mock 抑制重连");
        Ok(())
    }
}
