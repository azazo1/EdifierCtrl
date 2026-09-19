use std::str::FromStr;
use std::sync::Arc;

use async_trait::async_trait;
use bluer::{Adapter, Address, Device, Session, Uuid};
use edifier_protocol::RFCOMM_SERVICE_UUID;
use edifier_runtime::{HeadsetTransport, LinkKind, ScanResult, TransportError};
use tokio::sync::Mutex;
use tracing::info;

mod link;

type RfcommSession = link::Session<bluer::rfcomm::Stream>;

pub struct LinuxHeadset {
    adapter: Mutex<Option<Adapter>>,
    link: Mutex<Option<Arc<RfcommSession>>>,
}

impl LinuxHeadset {
    pub fn new() -> Self {
        Self {
            adapter: Mutex::new(None),
            link: Mutex::new(None),
        }
    }

    async fn adapter(&self) -> Result<Adapter, TransportError> {
        if let Some(a) = self.adapter.lock().await.clone() {
            return Ok(a);
        }
        let session = Session::new()
            .await
            .map_err(|e| TransportError::Unavailable(e.to_string()))?;
        let adapter = session
            .default_adapter()
            .await
            .map_err(|e| TransportError::Unavailable(e.to_string()))?;
        adapter
            .set_powered(true)
            .await
            .map_err(|e| TransportError::Unavailable(e.to_string()))?;
        *self.adapter.lock().await = Some(adapter.clone());
        Ok(adapter)
    }
}

impl Default for LinuxHeadset {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl HeadsetTransport for LinuxHeadset {
    async fn scan(&self, kind: LinkKind) -> Result<Vec<ScanResult>, TransportError> {
        if kind != LinkKind::Rfcomm {
            return Err(TransportError::Unsupported(
                "Linux 端先走 RFCOMM, BLE 稍后".into(),
            ));
        }
        let adapter = self.adapter().await?;
        let addrs = adapter
            .device_addresses()
            .await
            .map_err(|e| TransportError::Unavailable(e.to_string()))?;
        let want = Uuid::from_str(RFCOMM_SERVICE_UUID)
            .map_err(|e| TransportError::Connect(e.to_string()))?;
        let mut out = Vec::new();
        for addr in addrs {
            let dev = match adapter.device(addr) {
                Ok(d) => d,
                Err(_) => continue,
            };
            let uuids = dev.uuids().await.ok().flatten().unwrap_or_default();
            if !uuids.contains(&want) {
                continue;
            }
            let name = dev.name().await.ok().flatten().unwrap_or_default();
            info!(target: "edifier_bt_linux", %addr, %name, "扫描到 RFCOMM 耳机");
            out.push(ScanResult {
                address: addr.to_string().to_ascii_uppercase(),
                name,
                kind: LinkKind::Rfcomm,
                service_uuid: Some(RFCOMM_SERVICE_UUID.into()),
            });
        }
        Ok(out)
    }

    async fn open(&self, address: &str, kind: LinkKind) -> Result<(), TransportError> {
        if kind != LinkKind::Rfcomm {
            return Err(TransportError::Unsupported(
                "Linux 端先走 RFCOMM".into(),
            ));
        }
        let adapter = self.adapter().await?;
        let addr = Address::from_str(address)
            .map_err(|e| TransportError::NotFound(e.to_string()))?;
        let device = adapter
            .device(addr)
            .map_err(|e| TransportError::NotFound(e.to_string()))?;
        connect_device(&device).await?;
        let uuid = Uuid::from_str(RFCOMM_SERVICE_UUID)
            .map_err(|e| TransportError::Connect(e.to_string()))?;
        device
            .connect_profile(&uuid)
            .await
            .map_err(|e| TransportError::Connect(e.to_string()))?;
        let stream = open_rfcomm(addr).await?;
        if let Some(previous) = self.link.lock().await.replace(Arc::new(RfcommSession::new(stream))) {
            previous.close();
        }
        info!(target: "edifier_bt_linux", address, "RFCOMM 已连接");
        Ok(())
    }

    async fn write(&self, bytes: &[u8]) -> Result<(), TransportError> {
        let session = self.link.lock().await.clone().ok_or(TransportError::Closed)?;
        session.write(bytes).await
    }

    async fn recv(&self) -> Result<Vec<u8>, TransportError> {
        let session = self.link.lock().await.clone().ok_or(TransportError::Closed)?;
        session.recv().await
    }

    async fn close(&self) -> Result<(), TransportError> {
        if let Some(session) = self.link.lock().await.take() {
            session.close();
        }
        Ok(())
    }
}

async fn connect_device(device: &Device) -> Result<(), TransportError> {
    if !device
        .is_connected()
        .await
        .map_err(|e| TransportError::Connect(e.to_string()))?
    {
        device
            .connect()
            .await
            .map_err(|e| TransportError::Connect(e.to_string()))?;
    }
    Ok(())
}

async fn open_rfcomm(addr: Address) -> Result<bluer::rfcomm::Stream, TransportError> {
    let mut last = TransportError::Connect("RFCOMM 通道均失败".into());
    for channel in 1..=30 {
        let socket =
            bluer::rfcomm::Socket::new().map_err(|e| TransportError::Connect(e.to_string()))?;
        match socket
            .connect(bluer::rfcomm::SocketAddr::new(addr, channel))
            .await
        {
            Ok(stream) => return Ok(stream),
            Err(err) => last = TransportError::Connect(err.to_string()),
        }
    }
    Err(last)
}
