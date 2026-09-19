use std::sync::mpsc;
use std::time::Duration;

use async_trait::async_trait;
use edifier_runtime::{AudioControl, AudioState, TransportError};
use tokio::sync::Mutex;
use tracing::{info, warn};
use windows::core::Interface;
use windows::Devices::Bluetooth::BluetoothDevice;
use windows::Foundation::IClosable;
use windows::Media::Audio::AudioPlaybackConnection;
use windows::System::{DispatcherQueue, DispatcherQueueController, DispatcherQueueHandler};

use crate::a2dp;
use crate::com::{parse_addr, win_err};

/// 音频控制: 优先 Win32 A2DP 服务, WinRT 只在专用 DispatcherQueue 上调用.
pub struct WindowsAudio {
    claimed: Mutex<bool>,
    winrt: Mutex<Option<WinrtPlayback>>,
}

struct WinrtPlayback {
    queue: DispatcherQueue,
    _ctrl: DispatcherQueueController,
    conn: std::sync::Mutex<Option<AudioPlaybackConnection>>,
}

impl WindowsAudio {
    pub fn new() -> Self {
        Self {
            claimed: Mutex::new(false),
            winrt: Mutex::new(None),
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
        if *self.claimed.lock().await {
            Ok(AudioState::Connected)
        } else {
            Ok(AudioState::Unknown)
        }
    }

    async fn connect_audio(&self, address: &str) -> Result<(), TransportError> {
        let addr = address.to_string();
        match a2dp::set_a2dp(&addr, true) {
            Ok(()) => {
                *self.claimed.lock().await = true;
                let acl = a2dp::a2dp_connected(&addr).unwrap_or(false);
                info!(
                    target: "edifier_bt_windows",
                    address = %addr,
                    acl,
                    "Win32 已请求 A2DP 连接"
                );
                return Ok(());
            }
            Err(err) => {
                warn!(
                    target: "edifier_bt_windows",
                    %err,
                    address = %addr,
                    "Win32 A2DP 连接失败"
                );
            }
        }
        if std::env::var("EDIFIER_WIN_AUDIO").as_deref() == Ok("1") {
            self.connect_winrt(&addr).await?;
            *self.claimed.lock().await = true;
            return Ok(());
        }
        Err(TransportError::Unavailable(
            "Windows 无法在无界面线程连接 A2DP. 设 EDIFIER_WIN_AUDIO=1 会试 WinRT, 本机上会访问冲突".into(),
        ))
    }

    async fn disconnect_audio(&self, address: &str) -> Result<(), TransportError> {
        self.close_winrt().await;
        if let Err(err) = a2dp::set_a2dp(address, false) {
            warn!(target: "edifier_bt_windows", %err, address, "Win32 A2DP 断开失败");
        }
        *self.claimed.lock().await = false;
        info!(target: "edifier_bt_windows", address, "已请求断开 A2DP");
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
            "Windows 没有公开的 A2DP 抑制重连接口"
        );
        if suppress {
            let _ = self.disconnect_audio(address).await;
        }
        Ok(())
    }
}

impl WindowsAudio {
    async fn connect_winrt(&self, address: &str) -> Result<(), TransportError> {
        let addr = address.to_string();
        let queue = self.ensure_queue().await?;
        let conn = on_queue(&queue, move || open_playback(&addr))?;
        if let Some(playback) = self.winrt.lock().await.as_ref() {
            *playback
                .conn
                .lock()
                .map_err(|e| TransportError::Connect(e.to_string()))? = Some(conn);
        }
        info!(target: "edifier_bt_windows", address, "WinRT 音频已连接");
        Ok(())
    }

    async fn close_winrt(&self) {
        let guard = self.winrt.lock().await;
        let Some(playback) = guard.as_ref() else {
            return;
        };
        let queue = playback.queue.clone();
        let taken = playback.conn.lock().ok().and_then(|mut g| g.take());
        drop(guard);
        if let Some(conn) = taken {
            let _ = on_queue(&queue, move || close_playback(conn));
        }
    }

    async fn ensure_queue(&self) -> Result<DispatcherQueue, TransportError> {
        let mut slot = self.winrt.lock().await;
        if slot.is_none() {
            let ctrl = DispatcherQueueController::CreateOnDedicatedThread().map_err(win_err)?;
            let queue = ctrl.DispatcherQueue().map_err(win_err)?;
            *slot = Some(WinrtPlayback {
                queue,
                _ctrl: ctrl,
                conn: std::sync::Mutex::new(None),
            });
        }
        Ok(slot.as_ref().expect("刚写入").queue.clone())
    }
}

fn on_queue<T, F>(queue: &DispatcherQueue, f: F) -> Result<T, TransportError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, TransportError> + Send + 'static,
{
    let (tx, rx) = mpsc::channel();
    let mut job = Some(f);
    let handler = DispatcherQueueHandler::new(move || {
        if let Some(job) = job.take() {
            let _ = tx.send(job());
        }
        Ok(())
    });
    if !queue.TryEnqueue(&handler).map_err(win_err)? {
        return Err(TransportError::Connect("DispatcherQueue 拒绝入队".into()));
    }
    rx.recv_timeout(Duration::from_secs(20))
        .map_err(|e| TransportError::Connect(format!("等待 DispatcherQueue: {e}")))?
}

fn close_playback(conn: AudioPlaybackConnection) -> Result<(), TransportError> {
    let closable: IClosable = conn.cast().map_err(win_err)?;
    closable.Close().map_err(win_err)
}

fn open_playback(address: &str) -> Result<AudioPlaybackConnection, TransportError> {
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
    Ok(conn)
}
