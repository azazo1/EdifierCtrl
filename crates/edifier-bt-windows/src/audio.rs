use async_trait::async_trait;
use edifier_runtime::{AudioControl, AudioState, TransportError};
use tokio::sync::Mutex;
use tracing::{info, warn};
use windows::core::Interface;
use windows::Devices::Bluetooth::BluetoothDevice;
use windows::Foundation::IClosable;
use windows::Media::Audio::AudioPlaybackConnection;

use crate::com::{blocking, parse_addr, win_err, Mta};

pub struct WindowsAudio {
    current: Mutex<Option<Mta<AudioPlaybackConnection>>>,
}

impl WindowsAudio {
    pub fn new() -> Self {
        Self {
            current: Mutex::new(None),
        }
    }
}

impl Default for WindowsAudio {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AudioControl for WindowsAudio {
    async fn audio_state(&self, _address: &str) -> Result<AudioState, TransportError> {
        if self.current.lock().await.is_some() {
            Ok(AudioState::Connected)
        } else {
            Ok(AudioState::Unknown)
        }
    }

    async fn connect_audio(&self, address: &str) -> Result<(), TransportError> {
        if let Some(old) = self.current.lock().await.take() {
            let _ = blocking(move || close_playback(old)).await;
        }
        let addr = address.to_string();
        let conn = blocking({
            let addr = addr.clone();
            move || open_playback(&addr)
        })
        .await?;
        *self.current.lock().await = Some(conn);
        info!(target: "edifier_bt_windows", address = %addr, "音频已连接");
        Ok(())
    }

    async fn disconnect_audio(&self, address: &str) -> Result<(), TransportError> {
        let old = self.current.lock().await.take();
        if let Some(conn) = old {
            blocking(move || close_playback(conn)).await?;
        }
        info!(target: "edifier_bt_windows", address, "音频已断开");
        Ok(())
    }

    async fn suppress_autoreconnect(
        &self,
        address: &str,
        suppress: bool,
    ) -> Result<(), TransportError> {
        warn!(
            target: "edifier_bt_windows",
            address,
            suppress,
            "Windows 没有公开的 A2DP 抑制重连接口, 只能保持 AudioPlaybackConnection 关闭"
        );
        if suppress {
            if let Some(conn) = self.current.lock().await.take() {
                let _ = blocking(move || close_playback(conn)).await;
            }
        }
        Ok(())
    }
}

fn close_playback(conn: Mta<AudioPlaybackConnection>) -> Result<(), TransportError> {
    let closable: IClosable = conn.0.cast().map_err(win_err)?;
    closable.Close().map_err(win_err)
}

fn open_playback(address: &str) -> Result<Mta<AudioPlaybackConnection>, TransportError> {
    let addr = parse_addr(address)?;
    let device = BluetoothDevice::FromBluetoothAddressAsync(addr)
        .map_err(win_err)?
        .get()
        .map_err(win_err)?;
    if device.as_raw().is_null() {
        return Err(TransportError::NotFound(format!(
            "没有蓝牙设备 {address}"
        )));
    }
    let id = device.DeviceId().map_err(win_err)?;
    info!(target: "edifier_bt_windows", id = %id, address, "打开 AudioPlaybackConnection");
    let conn = AudioPlaybackConnection::TryCreateFromId(&id).map_err(win_err)?;
    if conn.as_raw().is_null() {
        return Err(TransportError::Unavailable(
            "该设备没有 AudioPlaybackConnection".into(),
        ));
    }
    conn.Start().map_err(win_err)?;
    let op = conn.OpenAsync().map_err(win_err)?;
    let _status = op.get().map_err(win_err)?;
    Ok(Mta(conn))
}
