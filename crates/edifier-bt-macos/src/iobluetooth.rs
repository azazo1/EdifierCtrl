//! 通过 dlsym 调用 Swift `@_cdecl` 的 IOBluetooth 桥.

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_void};

use async_trait::async_trait;
use edifier_protocol::RFCOMM_SERVICE_UUID;
use edifier_runtime::{
    AudioControl, AudioState, HeadsetTransport, LinkKind, ScanResult, TransportError,
};
use tracing::info;

type ScanFn = unsafe extern "C" fn() -> *mut c_char;
type OpenFn = unsafe extern "C" fn(*const c_char) -> c_int;
type WriteFn = unsafe extern "C" fn(*const u8, c_int) -> c_int;
type ReadFn = unsafe extern "C" fn(*mut u8, c_int) -> c_int;
type UnitFn = unsafe extern "C" fn() -> c_int;
type AddrFn = unsafe extern "C" fn(*const c_char) -> c_int;
type AddrStateFn = unsafe extern "C" fn(*const c_char) -> *mut c_char;
type ErrFn = unsafe extern "C" fn() -> *const c_char;

fn missing(name: &str) -> TransportError {
    TransportError::Unavailable(format!(
        "找不到 {name}. 需要 Swift 应用链接 IOBluetooth 桥"
    ))
}

fn last_error() -> String {
    unsafe {
        let Some(fn_) = load::<ErrFn>(c"edifier_macos_last_error") else {
            return "macOS 蓝牙桥未加载".into();
        };
        let p = fn_();
        if p.is_null() {
            "macOS 蓝牙失败".into()
        } else {
            CStr::from_ptr(p).to_string_lossy().into_owned()
        }
    }
}

fn load<T>(name: &CStr) -> Option<T> {
    unsafe {
        let ptr = libc::dlsym(libc::RTLD_DEFAULT, name.as_ptr());
        if ptr.is_null() {
            None
        } else {
            Some(std::mem::transmute_copy(&ptr))
        }
    }
}

fn take_json(ptr: *mut c_char) -> Result<String, TransportError> {
    if ptr.is_null() {
        return Err(TransportError::Unavailable(last_error()));
    }
    let text = unsafe { CStr::from_ptr(ptr) }
        .to_string_lossy()
        .into_owned();
    unsafe { libc::free(ptr as *mut c_void) };
    Ok(text)
}

pub struct MacosHeadset;

impl MacosHeadset {
    pub fn new() -> Self {
        Self
    }
}

impl Default for MacosHeadset {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl HeadsetTransport for MacosHeadset {
    async fn scan(&self, kind: LinkKind) -> Result<Vec<ScanResult>, TransportError> {
        if kind != LinkKind::Rfcomm {
            return Err(TransportError::Unsupported(
                "macOS 端先走已配对 RFCOMM".into(),
            ));
        }
        let json = tokio::task::spawn_blocking(|| unsafe {
            let fn_ = load::<ScanFn>(c"edifier_macos_scan").ok_or_else(|| missing("edifier_macos_scan"))?;
            take_json(fn_())
        })
        .await
        .map_err(|e| TransportError::Unavailable(e.to_string()))??;
        let rows: Vec<ScanRow> = serde_json::from_str(&json)
            .map_err(|e| TransportError::Unavailable(e.to_string()))?;
        Ok(rows
            .into_iter()
            .map(|r| ScanResult {
                address: r.address,
                name: r.name,
                kind: LinkKind::Rfcomm,
                service_uuid: Some(
                    r.service_uuid
                        .unwrap_or_else(|| RFCOMM_SERVICE_UUID.into()),
                ),
            })
            .collect())
    }

    async fn open(&self, address: &str, kind: LinkKind) -> Result<(), TransportError> {
        if kind != LinkKind::Rfcomm {
            return Err(TransportError::Unsupported("macOS 端先走 RFCOMM".into()));
        }
        let address = address.to_string();
        let for_log = address.clone();
        tokio::task::spawn_blocking(move || unsafe {
            let fn_ = load::<OpenFn>(c"edifier_macos_open").ok_or_else(|| missing("edifier_macos_open"))?;
            let c = CString::new(address).map_err(|e| TransportError::Connect(e.to_string()))?;
            if fn_(c.as_ptr()) == 0 {
                Ok(())
            } else {
                Err(TransportError::Connect(last_error()))
            }
        })
        .await
        .map_err(|e| TransportError::Unavailable(e.to_string()))??;
        info!(target: "edifier_bt_macos", address = %for_log, "RFCOMM 已连接");
        Ok(())
    }

    async fn write(&self, bytes: &[u8]) -> Result<(), TransportError> {
        let payload = bytes.to_vec();
        tokio::task::spawn_blocking(move || unsafe {
            let fn_ = load::<WriteFn>(c"edifier_macos_write").ok_or_else(|| missing("edifier_macos_write"))?;
            if fn_(payload.as_ptr(), payload.len() as c_int) == 0 {
                Ok(())
            } else {
                Err(TransportError::Write(last_error()))
            }
        })
        .await
        .map_err(|e| TransportError::Unavailable(e.to_string()))?
    }

    async fn recv(&self) -> Result<Vec<u8>, TransportError> {
        tokio::task::spawn_blocking(|| unsafe {
            let fn_ = load::<ReadFn>(c"edifier_macos_read").ok_or_else(|| missing("edifier_macos_read"))?;
            let mut buf = vec![0u8; 512];
            let n = fn_(buf.as_mut_ptr(), buf.len() as c_int);
            if n < 0 {
                Err(TransportError::Closed)
            } else {
                buf.truncate(n as usize);
                Ok(buf)
            }
        })
        .await
        .map_err(|e| TransportError::Unavailable(e.to_string()))?
    }

    async fn close(&self) -> Result<(), TransportError> {
        tokio::task::spawn_blocking(|| unsafe {
            let fn_ = load::<UnitFn>(c"edifier_macos_close").ok_or_else(|| missing("edifier_macos_close"))?;
            let _ = fn_();
            Ok(())
        })
        .await
        .map_err(|e| TransportError::Unavailable(e.to_string()))??;
        info!(target: "edifier_bt_macos", "已关闭 RFCOMM");
        Ok(())
    }
}

pub struct MacosAudio;

impl MacosAudio {
    pub fn new() -> Self {
        Self
    }
}

impl Default for MacosAudio {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AudioControl for MacosAudio {
    async fn audio_state(&self, address: &str) -> Result<AudioState, TransportError> {
        let address = address.to_string();
        let text = tokio::task::spawn_blocking(move || unsafe {
            let fn_ = load::<AddrStateFn>(c"edifier_macos_audio_state")
                .ok_or_else(|| missing("edifier_macos_audio_state"))?;
            let c = CString::new(address).map_err(|e| TransportError::Unavailable(e.to_string()))?;
            take_json(fn_(c.as_ptr()))
        })
        .await
        .map_err(|e| TransportError::Unavailable(e.to_string()))??;
        Ok(if text == "connected" {
            AudioState::Connected
        } else {
            AudioState::Disconnected
        })
    }

    async fn connect_audio(&self, address: &str) -> Result<(), TransportError> {
        audio_int(c"edifier_macos_audio_connect", address).await?;
        info!(target: "edifier_bt_macos", address, "已请求系统连接音频");
        Ok(())
    }

    async fn disconnect_audio(&self, address: &str) -> Result<(), TransportError> {
        audio_int(c"edifier_macos_audio_disconnect", address).await?;
        info!(target: "edifier_bt_macos", address, "已请求系统断开音频");
        Ok(())
    }

    async fn suppress_autoreconnect(
        &self,
        address: &str,
        suppress: bool,
    ) -> Result<(), TransportError> {
        if suppress {
            let _ = self.disconnect_audio(address).await;
        }
        Ok(())
    }
}

async fn audio_int(name: &'static CStr, address: &str) -> Result<(), TransportError> {
    let address = address.to_string();
    tokio::task::spawn_blocking(move || unsafe {
        let fn_ = load::<AddrFn>(name).ok_or_else(|| missing(&name.to_string_lossy()))?;
        let c = CString::new(address).map_err(|e| TransportError::Connect(e.to_string()))?;
        if fn_(c.as_ptr()) == 0 {
            Ok(())
        } else {
            Err(TransportError::Connect(last_error()))
        }
    })
    .await
    .map_err(|e| TransportError::Unavailable(e.to_string()))?
}

#[derive(serde::Deserialize)]
struct ScanRow {
    address: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    service_uuid: Option<String>,
}
