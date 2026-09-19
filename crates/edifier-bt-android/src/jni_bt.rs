//! 通过 JNI 调用 Kotlin `BluetoothBridge`.

use std::sync::OnceLock;

use async_trait::async_trait;
use edifier_protocol::RFCOMM_SERVICE_UUID;
use edifier_runtime::{
    AudioControl, AudioState, HeadsetTransport, LinkKind, ScanResult, TransportError,
};
use jni::objects::{GlobalRef, JByteArray, JClass, JObject, JString, JValue};
use jni::JavaVM;
use tracing::info;

static VM: OnceLock<JavaVM> = OnceLock::new();
static BRIDGE: OnceLock<GlobalRef> = OnceLock::new();

pub fn bind_jvm(vm: JavaVM) -> Result<(), String> {
    let mut env = vm.get_env().map_err(|e| e.to_string())?;
    let class = env
        .find_class("dev/edifierctrl/app/BluetoothBridge")
        .map_err(|e| format!("找不到 BluetoothBridge: {e}"))?;
    let global = env.new_global_ref(class).map_err(|e| e.to_string())?;
    let _ = BRIDGE.set(global);
    let _ = VM.set(vm);
    info!(target: "edifier_bt_android", "JNI 已绑定 BluetoothBridge");
    Ok(())
}

fn call<T, F>(f: F) -> Result<T, TransportError>
where
    F: FnOnce(&mut jni::JNIEnv) -> Result<T, String>,
{
    let vm = VM.get().ok_or_else(|| {
        TransportError::Unavailable("尚未 JNI_OnLoad, 先 System.loadLibrary".into())
    })?;
    let mut env = vm
        .attach_current_thread()
        .map_err(|e| TransportError::Unavailable(e.to_string()))?;
    f(&mut env).map_err(TransportError::Unavailable)
}

fn class_ref<'local>(env: &mut jni::JNIEnv<'local>) -> Result<JClass<'local>, String> {
    let g = BRIDGE.get().ok_or_else(|| "尚未绑定 BluetoothBridge".to_string())?;
    Ok(JClass::from(env.new_local_ref(g).map_err(|e| e.to_string())?))
}

fn check_exc(env: &mut jni::JNIEnv) -> Result<(), String> {
    if env.exception_check().unwrap_or(false) {
        let _ = env.exception_describe();
        let _ = env.exception_clear();
        return Err("BluetoothBridge 抛出异常".into());
    }
    Ok(())
}

fn last_bt_error(env: &mut jni::JNIEnv) -> String {
    let class = match class_ref(env) {
        Ok(c) => c,
        Err(e) => return e,
    };
    let ok = env.call_static_method(class, "lastError", "()Ljava/lang/String;", &[]);
    match ok {
        Ok(v) => {
            let obj = match v.l() {
                Ok(o) if !o.is_null() => o,
                _ => return "Android 蓝牙失败".into(),
            };
            let js = JString::from(obj);
            let text = env
                .get_string(&js)
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            if text.is_empty() {
                "Android 蓝牙失败".into()
            } else {
                text
            }
        }
        Err(_) => "Android 蓝牙失败".into(),
    }
}

fn call_int(name: &str, sig: &str, args: &[JValue]) -> Result<(), TransportError> {
    call(|env| {
        let class = class_ref(env)?;
        let rc = env
            .call_static_method(class, name, sig, args)
            .map_err(|e| e.to_string())?;
        check_exc(env)?;
        let n = rc.i().map_err(|e| e.to_string())?;
        if n == 0 {
            Ok(())
        } else {
            Err(last_bt_error(env))
        }
    })
}

fn call_string(name: &str) -> Result<String, TransportError> {
    call(|env| {
        let class = class_ref(env)?;
        let v = env
            .call_static_method(class, name, "()Ljava/lang/String;", &[])
            .map_err(|e| e.to_string())?;
        check_exc(env)?;
        let obj = v.l().map_err(|e| e.to_string())?;
        if obj.is_null() {
            return Err("BluetoothBridge 返回空".into());
        }
        let js = JString::from(obj);
        let java_str = env.get_string(&js).map_err(|e| e.to_string())?;
        Ok(java_str.to_string_lossy().into_owned())
    })
}

pub struct AndroidHeadset;

impl AndroidHeadset {
    pub fn new() -> Self {
        Self
    }
}

impl Default for AndroidHeadset {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl HeadsetTransport for AndroidHeadset {
    async fn scan(&self, kind: LinkKind) -> Result<Vec<ScanResult>, TransportError> {
        if kind != LinkKind::Rfcomm {
            return Err(TransportError::Unsupported(
                "Android 端先走已配对 RFCOMM".into(),
            ));
        }
        let json = tokio::task::spawn_blocking(|| call_string("scanRfcomm"))
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
            return Err(TransportError::Unsupported(
                "Android 端先走 RFCOMM".into(),
            ));
        }
        let address = address.to_string();
        let for_log = address.clone();
        tokio::task::spawn_blocking(move || {
            call(|env| {
                let class = class_ref(env)?;
                let jaddr = env.new_string(&address).map_err(|e| e.to_string())?;
                let rc = env
                    .call_static_method(
                        class,
                        "openRfcomm",
                        "(Ljava/lang/String;)I",
                        &[JValue::Object(&JObject::from(jaddr))],
                    )
                    .map_err(|e| e.to_string())?;
                check_exc(env)?;
                if rc.i().map_err(|e| e.to_string())? == 0 {
                    Ok(())
                } else {
                    Err(last_bt_error(env))
                }
            })
        })
        .await
        .map_err(|e| TransportError::Unavailable(e.to_string()))??;
        info!(target: "edifier_bt_android", address = %for_log, "RFCOMM 已连接");
        Ok(())
    }

    async fn write(&self, bytes: &[u8]) -> Result<(), TransportError> {
        let payload = bytes.to_vec();
        tokio::task::spawn_blocking(move || {
            call(|env| {
                let class = class_ref(env)?;
                let arr = env
                    .byte_array_from_slice(&payload)
                    .map_err(|e| e.to_string())?;
                let rc = env
                    .call_static_method(
                        class,
                        "writeRfcomm",
                        "([B)I",
                        &[JValue::Object(&JObject::from(arr))],
                    )
                    .map_err(|e| e.to_string())?;
                check_exc(env)?;
                if rc.i().map_err(|e| e.to_string())? == 0 {
                    Ok(())
                } else {
                    Err(last_bt_error(env))
                }
            })
        })
        .await
        .map_err(|e| TransportError::Unavailable(e.to_string()))?
    }

    async fn recv(&self) -> Result<Vec<u8>, TransportError> {
        tokio::task::spawn_blocking(|| {
            call(|env| {
                let class = class_ref(env)?;
                let v = env
                    .call_static_method(class, "readRfcomm", "(I)[B", &[JValue::Int(512)])
                    .map_err(|e| e.to_string())?;
                check_exc(env)?;
                let obj = v.l().map_err(|e| e.to_string())?;
                if obj.is_null() {
                    return Ok(Vec::new());
                }
                let arr = JByteArray::from(obj);
                let bytes = env.convert_byte_array(&arr).map_err(|e| e.to_string())?;
                Ok(bytes)
            })
        })
        .await
        .map_err(|e| TransportError::Unavailable(e.to_string()))?
    }

    async fn close(&self) -> Result<(), TransportError> {
        tokio::task::spawn_blocking(|| call_int("closeRfcomm", "()I", &[]))
            .await
            .map_err(|e| TransportError::Unavailable(e.to_string()))??;
        info!(target: "edifier_bt_android", "已关闭 RFCOMM");
        Ok(())
    }
}

pub struct AndroidAudio;

impl AndroidAudio {
    pub fn new() -> Self {
        Self
    }
}

impl Default for AndroidAudio {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AudioControl for AndroidAudio {
    async fn audio_state(&self, address: &str) -> Result<AudioState, TransportError> {
        let address = address.to_string();
        let text = tokio::task::spawn_blocking(move || {
            call(|env| {
                let class = class_ref(env)?;
                let jaddr = env.new_string(&address).map_err(|e| e.to_string())?;
                let v = env
                    .call_static_method(
                        class,
                        "audioState",
                        "(Ljava/lang/String;)Ljava/lang/String;",
                        &[JValue::Object(&JObject::from(jaddr))],
                    )
                    .map_err(|e| e.to_string())?;
                check_exc(env)?;
                let obj = v.l().map_err(|e| e.to_string())?;
                if obj.is_null() {
                    return Ok("unknown".into());
                }
                Ok(env
                    .get_string(&JString::from(obj))
                    .map_err(|e| e.to_string())?
                    .to_string_lossy()
                    .into_owned())
            })
        })
        .await
        .map_err(|e| TransportError::Unavailable(e.to_string()))??;
        Ok(match text.as_str() {
            "connected" => AudioState::Connected,
            "connecting" => AudioState::Connecting,
            "disconnected" => AudioState::Disconnected,
            _ => AudioState::Unknown,
        })
    }

    async fn connect_audio(&self, address: &str) -> Result<(), TransportError> {
        audio_int("connectAudio", address).await?;
        info!(target: "edifier_bt_android", address, "A2DP 已连接");
        Ok(())
    }

    async fn disconnect_audio(&self, address: &str) -> Result<(), TransportError> {
        audio_int("disconnectAudio", address).await?;
        info!(target: "edifier_bt_android", address, "A2DP 已断开");
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

async fn audio_int(name: &'static str, address: &str) -> Result<(), TransportError> {
    let address = address.to_string();
    tokio::task::spawn_blocking(move || {
        call(|env| {
            let class = class_ref(env)?;
            let jaddr = env.new_string(&address).map_err(|e| e.to_string())?;
            let rc = env
                .call_static_method(
                    class,
                    name,
                    "(Ljava/lang/String;)I",
                    &[JValue::Object(&JObject::from(jaddr))],
                )
                .map_err(|e| e.to_string())?;
            check_exc(env)?;
            if rc.i().map_err(|e| e.to_string())? == 0 {
                Ok(())
            } else {
                Err(last_bt_error(env))
            }
        })
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
