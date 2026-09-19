//! 会话 C ABI. `unsafe` 函数的句柄必须来自 `edifier_session_new` 且尚未 free.
#![allow(clippy::missing_safety_doc)]

use std::os::raw::{c_char, c_int};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use edifier_protocol::DeviceProfile;
use edifier_runtime::{
    AudioControl, CdFallback, GroupHub, HeadsetHost, LinkKind, RuntimeEvent, TransportError,
    UdpGroupNet,
};
use tokio::runtime::Runtime;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tracing::{info, warn};

use crate::command_json::CommandJson;
use crate::cstr::{fail, set_error, to_raw};
use crate::event_json::{event_json, scan_json, EventJson};
use crate::platform::{new_audio, new_headset, PlatformAudio, PlatformHeadset};

type GroupHandle = Arc<GroupHub<UdpGroupNet, Arc<PlatformAudio>>>;

pub struct FfiSession {
    rt: Runtime,
    host: Arc<HeadsetHost<PlatformHeadset>>,
    audio: Arc<PlatformAudio>,
    host_events: Mutex<broadcast::Receiver<RuntimeEvent>>,
    group: Mutex<Option<GroupHandle>>,
    group_events: Mutex<Option<broadcast::Receiver<RuntimeEvent>>>,
    tasks: Mutex<Vec<JoinHandle<()>>>,
    local_id: String,
    headset_addr: Mutex<Option<String>>,
}

impl FfiSession {
    fn new(local_id: String) -> Result<Self, String> {
        let rt = Runtime::new().map_err(|e| e.to_string())?;
        let host = Arc::new(HeadsetHost::new(new_headset()));
        let audio = Arc::new(new_audio());
        let host_events = Mutex::new(host.subscribe());
        let pump = host.clone();
        let handle = rt.spawn(async move {
            loop {
                let _ = pump.pump().await;
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        });
        info!(target: "edifier_ffi", id = %local_id, "会话已创建");
        Ok(Self {
            rt,
            host,
            audio,
            host_events,
            group: Mutex::new(None),
            group_events: Mutex::new(None),
            tasks: Mutex::new(vec![handle]),
            local_id,
            headset_addr: Mutex::new(None),
        })
    }
}

fn parse_kind(kind: &str) -> Result<LinkKind, String> {
    match kind {
        "rfcomm" => Ok(LinkKind::Rfcomm),
        "ble" => Ok(LinkKind::Ble),
        _ => Err("kind 只能是 rfcomm 或 ble".into()),
    }
}

fn session_ref<'a>(ptr: *mut FfiSession) -> Result<&'a FfiSession, String> {
    if ptr.is_null() {
        Err("空会话".into())
    } else {
        Ok(unsafe { &*ptr })
    }
}

fn json_ok<T: serde::Serialize>(value: T) -> *mut c_char {
    match serde_json::to_string(&value) {
        Ok(text) => to_raw(text),
        Err(err) => fail(err.to_string()),
    }
}

fn unit_ok(result: Result<(), String>) -> c_int {
    match result {
        Ok(()) => 0,
        Err(err) => {
            set_error(err);
            -1
        }
    }
}

fn map_err(err: TransportError) -> String {
    err.to_string()
}

#[no_mangle]
pub extern "C" fn edifier_session_new(local_id: *const c_char) -> *mut FfiSession {
    let id = crate::cstr::from_ptr(local_id)
        .ok()
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_else(default_local_id);
    match FfiSession::new(id) {
        Ok(session) => Box::into_raw(Box::new(session)),
        Err(err) => {
            set_error(err);
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn edifier_session_free(session: *mut FfiSession) {
    if session.is_null() {
        return;
    }
    let boxed = unsafe { Box::from_raw(session) };
    if let Ok(mut tasks) = boxed.tasks.lock() {
        for task in tasks.drain(..) {
            task.abort();
        }
    }
    info!(target: "edifier_ffi", "会话已释放");
}

#[no_mangle]
pub extern "C" fn edifier_session_scan(
    session: *mut FfiSession,
    kind: *const c_char,
) -> *mut c_char {
    (|| {
        let s = session_ref(session)?;
        let kind = parse_kind(crate::cstr::from_ptr(kind).map_err(str::to_string)?)?;
        let list = s.rt.block_on(s.host.scan(kind)).map_err(map_err)?;
        serde_json::to_string(&scan_json(&list)).map_err(|e| e.to_string())
    })()
    .map(to_raw)
    .unwrap_or_else(fail)
}

#[no_mangle]
pub extern "C" fn edifier_session_connect(
    session: *mut FfiSession,
    address: *const c_char,
    kind: *const c_char,
) -> c_int {
    unit_ok((|| {
        let s = session_ref(session)?;
        let address = crate::cstr::from_ptr(address).map_err(str::to_string)?;
        let kind = parse_kind(crate::cstr::from_ptr(kind).map_err(str::to_string)?)?;
        s.rt.block_on(s.host.connect(address, kind)).map_err(map_err)?;
        *s.headset_addr.lock().map_err(|e| e.to_string())? = Some(address.to_string());
        sync_holding(s)
    })())
}

#[no_mangle]
pub extern "C" fn edifier_session_disconnect(session: *mut FfiSession) -> c_int {
    unit_ok((|| {
        let s = session_ref(session)?;
        let mac = {
            let mut addr = s.headset_addr.lock().map_err(|e| e.to_string())?;
            let old = addr.clone();
            *addr = None;
            old
        };
        s.rt.block_on(s.host.disconnect()).map_err(map_err)?;
        if let Some(hub) = s.group.lock().map_err(|e| e.to_string())?.clone() {
            s.rt.block_on(hub.adopt_headset(None));
        } else if let Some(mac) = mac {
            if let Err(err) = s.rt.block_on(s.audio.disconnect_audio(&mac)) {
                warn!(
                    target: "edifier_ffi",
                    address = %mac,
                    error = %err,
                    "未能断开 A2DP"
                );
            }
        }
        Ok(())
    })())
}

#[no_mangle]
pub extern "C" fn edifier_session_readout(
    session: *mut FfiSession,
    profile_key: *const c_char,
) -> c_int {
    unit_ok((|| {
        let s = session_ref(session)?;
        let key = crate::cstr::from_ptr(profile_key)
            .ok()
            .filter(|k| !k.is_empty())
            .unwrap_or("basedevice");
        let profile =
            DeviceProfile::by_key(key).ok_or_else(|| format!("未知机型档案: {key}"))?;
        s.rt.block_on(s.host.readout(profile)).map_err(map_err)
    })())
}

#[no_mangle]
pub extern "C" fn edifier_session_send_json(
    session: *mut FfiSession,
    command_json: *const c_char,
) -> c_int {
    unit_ok((|| {
        let s = session_ref(session)?;
        let text = crate::cstr::from_ptr(command_json).map_err(str::to_string)?;
        let json: CommandJson = serde_json::from_str(text).map_err(|e| e.to_string())?;
        let cmd = json.to_command()?;
        s.rt.block_on(s.host.send(&cmd)).map_err(map_err)
    })())
}

#[no_mangle]
pub extern "C" fn edifier_session_poll_event(session: *mut FfiSession) -> *mut c_char {
    match session_ref(session) {
        Ok(s) => json_ok(poll_event(s)),
        Err(err) => fail(err),
    }
}

fn poll_event(s: &FfiSession) -> EventJson {
    if let Ok(mut rx) = s.host_events.lock() {
        if let Ok(ev) = rx.try_recv() {
            return event_json(&ev);
        }
    }
    if let Ok(mut slot) = s.group_events.lock() {
        if let Some(rx) = slot.as_mut() {
            if let Ok(ev) = rx.try_recv() {
                return event_json(&ev);
            }
        }
    }
    EventJson::Empty
}

#[no_mangle]
pub extern "C" fn edifier_session_group_join(
    session: *mut FfiSession,
    passphrase: *const c_char,
) -> c_int {
    unit_ok((|| {
        let s = session_ref(session)?;
        if s.group.lock().map_err(|e| e.to_string())?.is_some() {
            return Err("已经加入组".into());
        }
        let pass = crate::cstr::from_ptr(passphrase).map_err(str::to_string)?;
        let net = s.rt.block_on(UdpGroupNet::bind()).map_err(map_err)?;
        let hub = Arc::new(GroupHub::new(
            pass,
            s.local_id.clone(),
            net,
            s.audio.clone(),
            Some(s.host.clone() as Arc<dyn CdFallback>),
        ));
        *s.group_events.lock().map_err(|e| e.to_string())? = Some(hub.subscribe());
        *s.group.lock().map_err(|e| e.to_string())? = Some(hub.clone());
        let run = hub.clone();
        let handle = s.rt.spawn(async move {
            let _ = run.run().await;
        });
        s.tasks.lock().map_err(|e| e.to_string())?.push(handle);
        info!(
            target: "edifier_ffi",
            group = %hub.group_id_hex(),
            "已加入局域网组"
        );
        sync_holding(s)
    })())
}

#[no_mangle]
pub extern "C" fn edifier_session_group_peers(session: *mut FfiSession) -> *mut c_char {
    (|| {
        let s = session_ref(session)?;
        let hub = hub_of(s)?;
        let peers = s.rt.block_on(hub.peers());
        serde_json::to_string(&peers).map_err(|e| e.to_string())
    })()
    .map(to_raw)
    .unwrap_or_else(fail)
}

fn hub_of(s: &FfiSession) -> Result<GroupHandle, String> {
    s.group
        .lock()
        .map_err(|e| e.to_string())?
        .clone()
        .ok_or_else(|| "尚未加入组".to_string())
}

#[no_mangle]
pub extern "C" fn edifier_session_group_claim(
    session: *mut FfiSession,
    mac: *const c_char,
) -> c_int {
    unit_ok((|| {
        let s = session_ref(session)?;
        let mac = crate::cstr::from_ptr(mac).map_err(str::to_string)?;
        let hub = hub_of(s)?;
        s.rt.block_on(hub.claim(mac)).map_err(map_err)
    })())
}

#[no_mangle]
pub extern "C" fn edifier_session_group_claim_peer(
    session: *mut FfiSession,
    peer_id: *const c_char,
) -> c_int {
    unit_ok((|| {
        let s = session_ref(session)?;
        let peer_id = crate::cstr::from_ptr(peer_id).map_err(str::to_string)?;
        let hub = hub_of(s)?;
        let peers = s.rt.block_on(hub.peers());
        let peer = peers
            .iter()
            .find(|p| p.id == peer_id)
            .ok_or_else(|| "组里没有这个成员".to_string())?;
        let mac = peer
            .holding
            .as_deref()
            .ok_or_else(|| "该成员没有持有耳机".to_string())?;
        info!(
            target: "edifier_ffi",
            peer = peer_id,
            mac,
            "点成员接管音频"
        );
        s.rt.block_on(hub.claim(mac)).map_err(map_err)
    })())
}

#[no_mangle]
pub extern "C" fn edifier_session_group_id_hex(session: *mut FfiSession) -> *mut c_char {
    match session_ref(session).and_then(|s| {
        s.group
            .lock()
            .map_err(|e| e.to_string())?
            .as_ref()
            .map(|h| h.group_id_hex())
            .ok_or_else(|| "尚未加入组".to_string())
    }) {
        Ok(id) => to_raw(id),
        Err(err) => fail(err),
    }
}

fn sync_holding(s: &FfiSession) -> Result<(), String> {
    let mac = s
        .headset_addr
        .lock()
        .map_err(|e| e.to_string())?
        .clone();
    if let Some(hub) = s.group.lock().map_err(|e| e.to_string())?.clone() {
        s.rt.block_on(hub.adopt_headset(mac));
        return Ok(());
    }
    if let Some(mac) = mac {
        if let Err(err) = s.rt.block_on(s.audio.connect_audio(&mac)) {
            warn!(
                target: "edifier_ffi",
                address = %mac,
                error = %err,
                "未能连接 A2DP"
            );
        }
    }
    Ok(())
}

#[no_mangle]
pub extern "C" fn edifier_session_set_holding(
    session: *mut FfiSession,
    mac: *const c_char,
) -> c_int {
    unit_ok((|| {
        let s = session_ref(session)?;
        let value = crate::cstr::from_ptr(mac)
            .ok()
            .filter(|v| !v.is_empty())
            .map(str::to_string);
        *s.headset_addr.lock().map_err(|e| e.to_string())? = value;
        sync_holding(s)
    })())
}

#[no_mangle]
pub extern "C" fn edifier_session_holding(session: *mut FfiSession) -> *mut c_char {
    match session_ref(session).and_then(|s| {
        s.headset_addr
            .lock()
            .map_err(|e| e.to_string())
            .map(|g| g.clone().unwrap_or_default())
    }) {
        Ok(text) => to_raw(text),
        Err(err) => fail(err),
    }
}

fn default_local_id() -> String {
    let host = std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "host".into());
    format!("{}-{}", host, std::process::id())
}
