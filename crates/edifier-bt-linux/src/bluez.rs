use std::str::FromStr;

use async_trait::async_trait;
use bluer::{Adapter, Address, Device, Session, Uuid};
use edifier_protocol::RFCOMM_SERVICE_UUID;
use edifier_runtime::{
    AudioControl, AudioState, HeadsetTransport, LinkKind, ScanResult, TransportError,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex;
use tracing::{info, warn};

struct RfcommSession {
    stream: bluer::rfcomm::Stream,
}

pub struct LinuxHeadset {
    adapter: Mutex<Option<Adapter>>,
    link: Mutex<Option<RfcommSession>>,
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
        *self.link.lock().await = Some(RfcommSession { stream });
        info!(target: "edifier_bt_linux", address, "RFCOMM 已连接");
        Ok(())
    }

    async fn write(&self, bytes: &[u8]) -> Result<(), TransportError> {
        let mut link = self.link.lock().await;
        let sess = link
            .as_mut()
            .ok_or_else(|| TransportError::Connect("尚未连接".into()))?;
        sess.stream
            .write_all(bytes)
            .await
            .map_err(|e| TransportError::Write(e.to_string()))?;
        Ok(())
    }

    async fn recv(&self) -> Result<Vec<u8>, TransportError> {
        let mut buf = vec![0u8; 512];
        let mut link = self.link.lock().await;
        let sess = link.as_mut().ok_or(TransportError::Closed)?;
        let n = sess
            .stream
            .read(&mut buf)
            .await
            .map_err(|e| TransportError::Connect(e.to_string()))?;
        if n == 0 {
            return Err(TransportError::Closed);
        }
        buf.truncate(n);
        Ok(buf)
    }

    async fn close(&self) -> Result<(), TransportError> {
        *self.link.lock().await = None;
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

pub struct LinuxAudio {
    connected: Mutex<bool>,
}

impl LinuxAudio {
    pub fn new() -> Self {
        Self {
            connected: Mutex::new(false),
        }
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
        let address = address.to_string();
        let connected = tokio::task::spawn_blocking(move || bluetoothctl_connected(&address))
            .await
            .map_err(|e| TransportError::Unavailable(e.to_string()))??;
        *self.connected.lock().await = connected;
        Ok(if connected {
            AudioState::Connected
        } else {
            AudioState::Disconnected
        })
    }

    async fn connect_audio(&self, address: &str) -> Result<(), TransportError> {
        let addr = address.to_string();
        tokio::task::spawn_blocking({
            let addr = addr.clone();
            move || bluetoothctl(&["connect", &addr])
        })
        .await
        .map_err(|e| TransportError::Unavailable(e.to_string()))??;
        *self.connected.lock().await = true;
        info!(target: "edifier_bt_linux", address = %addr, "bluetoothctl connect");
        Ok(())
    }

    async fn disconnect_audio(&self, address: &str) -> Result<(), TransportError> {
        let addr = address.to_string();
        tokio::task::spawn_blocking({
            let addr = addr.clone();
            move || bluetoothctl(&["disconnect", &addr])
        })
        .await
        .map_err(|e| TransportError::Unavailable(e.to_string()))??;
        *self.connected.lock().await = false;
        info!(target: "edifier_bt_linux", address = %addr, "bluetoothctl disconnect");
        Ok(())
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
            "A2DP 抑制重连依赖 bluetoothd 策略, 当前仅记录状态"
        );
        Ok(())
    }
}

fn bluetoothctl(args: &[&str]) -> Result<(), TransportError> {
    let out = std::process::Command::new("bluetoothctl")
        .args(args)
        .output()
        .map_err(|e| TransportError::Unavailable(format!("bluetoothctl: {e}")))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        let stdout = String::from_utf8_lossy(&out.stdout);
        return Err(TransportError::Connect(format!("{err} {stdout}")));
    }
    Ok(())
}

fn bluetoothctl_connected(address: &str) -> Result<bool, TransportError> {
    let out = std::process::Command::new("bluetoothctl")
        .args(["info", address])
        .output()
        .map_err(|e| TransportError::Unavailable(format!("bluetoothctl: {e}")))?;
    let text = String::from_utf8_lossy(&out.stdout);
    Ok(text.lines().any(|l| l.contains("Connected: yes")))
}
