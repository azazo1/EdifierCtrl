use std::sync::mpsc as std_mpsc;

use async_trait::async_trait;
use edifier_runtime::{HeadsetTransport, LinkKind, ScanResult, TransportError};
use tokio::sync::{mpsc, Mutex};
use tracing::info;
use crate::ble::{self, BleLink};
use crate::com::blocking;
use crate::io;
use crate::rfcomm;
use windows::Networking::Sockets::StreamSocket;

use crate::com::Mta;

struct Inner {
    writer: Option<std_mpsc::Sender<Vec<u8>>>,
    _ble: Option<BleLink>,
    socket: Option<Mta<StreamSocket>>,
}

pub struct WindowsHeadset {
    inner: Mutex<Inner>,
    reader: Mutex<Option<mpsc::UnboundedReceiver<Vec<u8>>>>,
}

impl WindowsHeadset {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(Inner {
                writer: None,
                _ble: None,
                socket: None,
            }),
            reader: Mutex::new(None),
        }
    }
}

impl Default for WindowsHeadset {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl HeadsetTransport for WindowsHeadset {
    async fn scan(&self, kind: LinkKind) -> Result<Vec<ScanResult>, TransportError> {
        info!(target: "edifier_bt_windows", ?kind, "WinRT 扫描");
        match kind {
            LinkKind::Rfcomm => blocking(rfcomm::scan).await,
            LinkKind::Ble => blocking(ble::scan).await,
        }
    }

    async fn open(&self, address: &str, kind: LinkKind) -> Result<(), TransportError> {
        let address = address.to_string();
        match kind {
            LinkKind::Rfcomm => {
                let pipe = blocking(move || {
                    let socket = rfcomm::connect(&address)?;
                    io::spawn_socket(socket)
                })
                .await?;
                let mut inner = self.inner.lock().await;
                inner.writer = Some(pipe.writes);
                inner._ble = None;
                inner.socket = Some(pipe.socket);
                *self.reader.lock().await = Some(pipe.reads);
            }
            LinkKind::Ble => {
                let opened = blocking(move || {
                    let mut link = ble::connect(&address)?;
                    let reads = link
                        .reads
                        .take()
                        .ok_or_else(|| TransportError::Connect("BLE 接收通道丢失".into()))?;
                    let writes = ble::spawn_writer(link.writes.clone())?;
                    Ok((writes, reads, link))
                })
                .await?;
                let mut inner = self.inner.lock().await;
                inner.writer = Some(opened.0);
                inner._ble = Some(opened.2);
                inner.socket = None;
                *self.reader.lock().await = Some(opened.1);
            }
        }
        Ok(())
    }

    async fn write(&self, bytes: &[u8]) -> Result<(), TransportError> {
        let inner = self.inner.lock().await;
        match inner.writer.as_ref() {
            Some(tx) => tx
                .send(bytes.to_vec())
                .map_err(|_| TransportError::Write("写线程已退出".into())),
            None => Err(TransportError::Connect("尚未连接".into())),
        }
    }

    async fn recv(&self) -> Result<Vec<u8>, TransportError> {
        let mut reader = self.reader.lock().await;
        let rx = reader.as_mut().ok_or(TransportError::Closed)?;
        rx.recv().await.ok_or(TransportError::Closed)
    }

    async fn close(&self) -> Result<(), TransportError> {
        {
            let mut inner = self.inner.lock().await;
            inner.writer = None;
            inner._ble = None;
            if let Some(socket) = inner.socket.take() {
                socket.close();
            }
        }
        *self.reader.lock().await = None;
        info!(target: "edifier_bt_windows", "已关闭控制通道");
        Ok(())
    }
}
