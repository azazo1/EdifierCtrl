//! 会话 C ABI. `unsafe` 函数的句柄必须来自 `edifier_session_new` 且尚未 free.
#![allow(clippy::missing_safety_doc)]

use std::os::raw::{c_char, c_int};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use edifier_group::MacAddr;
use edifier_protocol::DeviceProfile;
use edifier_runtime::{
    AudioControl, AudioState, CdFallback, GroupHub, HeadsetHost, LinkKind, RuntimeEvent, TransportError,
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
    group: Arc<Mutex<Option<GroupHandle>>>,
    group_events: Mutex<Option<broadcast::Receiver<RuntimeEvent>>>,
    host_task: Mutex<Option<JoinHandle<()>>>,
    group_task: Mutex<Option<JoinHandle<()>>>,
    local_id: String,
    headset_addr: Mutex<Option<String>>,
}

impl FfiSession {
    fn new(local_id: String) -> Result<Self, String> {
        let rt = Runtime::new().map_err(|e| e.to_string())?;
        let host = Arc::new(HeadsetHost::new(new_headset()));
        let audio = Arc::new(new_audio());
        let host_events = Mutex::new(host.subscribe());
        info!(target: "edifier_ffi", id = %local_id, "会话已创建");
        Ok(Self {
            rt,
            host,
            audio,
            host_events,
            group: Arc::new(Mutex::new(None)),
            group_events: Mutex::new(None),
            host_task: Mutex::new(None),
            group_task: Mutex::new(None),
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
    if let Err(err) = leave_group(&boxed) {
        warn!(target: "edifier_ffi", %err, "退出组时发生错误");
    }
    stop_task(&boxed.rt, &boxed.host_task);
    if boxed.rt.block_on(boxed.host.connected()) {
        if let Err(err) = boxed.rt.block_on(boxed.host.disconnect()) {
            warn!(target: "edifier_ffi", %err, "销毁会话时关闭控制通道失败");
        }
    }
    info!(target: "edifier_ffi", "会话已释放");
    boxed.rt.shutdown_timeout(Duration::from_secs(1));
}

#[no_mangle]
pub unsafe extern "C" fn edifier_session_destroy(session: *mut FfiSession) {
    unsafe { edifier_session_free(session) };
}

fn stop_task(rt: &Runtime, slot: &Mutex<Option<JoinHandle<()>>>) {
    let task = slot.lock().unwrap_or_else(|e| e.into_inner()).take();
    if let Some(task) = task {
        task.abort();
        let _ = rt.block_on(task);
    }
}

fn start_pump(s: &FfiSession) {
    let host = s.host.clone();
    let group = s.group.clone();
    let task = s.rt.spawn(async move {
        if let Err(err) = host.pump().await {
            warn!(target: "edifier_ffi", %err, "控制通道接收结束");
        }
        let hub = group.lock().unwrap_or_else(|e| e.into_inner()).clone();
        if let Some(hub) = hub {
            hub.set_control_address(None).await;
        }
    });
    *s.host_task.lock().unwrap_or_else(|e| e.into_inner()) = Some(task);
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
        let address = MacAddr::parse(address).map(|mac| mac.to_colon_string())
            .unwrap_or_else(|_| address.to_string());
        // 换设备前撤销 CD 目标, 避免组任务向新通道发送旧耳机的释放命令.
        if let Ok(hub) = hub_of(s) {
            s.rt.block_on(hub.set_control_address(None));
        }
        stop_task(&s.rt, &s.host_task);
        if let Err(err) = s.rt.block_on(s.host.connect(&address, kind)) {
            if let Ok(hub) = hub_of(s) {
                s.rt.block_on(hub.set_control_address(None));
            }
            return Err(map_err(err));
        }
        *s.headset_addr.lock().map_err(|e| e.to_string())? = Some(address);
        start_pump(s);
        sync_holding(s)
    })())
}

#[no_mangle]
pub extern "C" fn edifier_session_disconnect(session: *mut FfiSession) -> c_int {
    unit_ok((|| {
        let s = session_ref(session)?;
        if let Ok(hub) = hub_of(s) {
            s.rt.block_on(hub.set_control_address(None));
        }
        stop_task(&s.rt, &s.host_task);
        s.rt.block_on(s.host.disconnect()).map_err(map_err)
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
        if pass.trim().is_empty() {
            return Err("组口令不能为空".into());
        }
        let net = s.rt.block_on(UdpGroupNet::bind()).map_err(map_err)?;
        let hub = Arc::new(GroupHub::new(
            pass,
            s.local_id.clone(),
            net,
            s.audio.clone(),
            Some(s.host.clone() as Arc<dyn CdFallback>),
        ));
        // 初始化成功后再发布给其它入口, 防止订阅或后台任务只创建一半.
        let control_address = s.rt.block_on(s.host.connected_address());
        s.rt.block_on(hub.set_control_address(control_address));
        let mac = s.headset_addr.lock().map_err(|e| e.to_string())?.clone();
        s.rt.block_on(hub.adopt_headset(mac));
        let mut group = s.group.lock().map_err(|e| e.to_string())?;
        let mut events = s.group_events.lock().map_err(|e| e.to_string())?;
        let mut task = s.group_task.lock().map_err(|e| e.to_string())?;
        *events = Some(hub.subscribe());
        *group = Some(hub.clone());
        let run = hub.clone();
        *task = Some(s.rt.spawn(async move {
            if let Err(err) = run.run().await {
                warn!(target: "edifier_ffi", %err, "局域网组接收结束");
                let _ = run.leave().await;
            }
        }));
        info!(
            target: "edifier_ffi",
            group = %hub.group_id_hex(),
            "已加入局域网组"
        );
        Ok(())
    })())
}

#[no_mangle]
pub extern "C" fn edifier_session_group_leave(session: *mut FfiSession) -> c_int {
    unit_ok(session_ref(session).and_then(leave_group))
}

fn leave_group(s: &FfiSession) -> Result<(), String> {
    let hub = s.group.lock().map_err(|e| e.to_string())?.take();
    if let Some(address) = hub.as_ref().and_then(|hub| s.rt.block_on(hub.holding())) {
        *s.headset_addr.lock().map_err(|e| e.to_string())? = Some(address);
    }
    // 先让正在执行的动作完成/回滚, 再取消 UDP runner 并释放 socket.
    let result = hub.as_ref().map(|hub| s.rt.block_on(hub.leave()).map_err(map_err))
        .unwrap_or(Ok(()));
    stop_task(&s.rt, &s.group_task);
    *s.group_events.lock().map_err(|e| e.to_string())? = None;
    if result.is_err() {
        // 保留关闭的 hub 供下一次 leave 重试平台清理, 禁止带着未恢复状态重新入组.
        *s.group.lock().map_err(|e| e.to_string())? = hub;
    }
    info!(target: "edifier_ffi", "已退出局域网组");
    result
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
    let group = s.group.lock().map_err(|e| e.to_string())?.clone();
    if let Some(hub) = group {
        let control_address = s.rt.block_on(s.host.connected_address());
        s.rt.block_on(hub.set_control_address(control_address));
        s.rt.block_on(hub.adopt_headset(mac));
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
        let value = if mac.is_null() {
            None
        } else {
            let text = crate::cstr::from_ptr(mac).map_err(str::to_string)?;
            if text.is_empty() {
                None
            } else {
                Some(MacAddr::parse(text).map_err(|e| e.to_string())?.to_colon_string())
            }
        };
        *s.headset_addr.lock().map_err(|e| e.to_string())? = value;
        sync_holding(s)
    })())
}

#[no_mangle]
pub extern "C" fn edifier_session_holding(session: *mut FfiSession) -> *mut c_char {
    match session_ref(session).and_then(|s| {
        if let Ok(hub) = hub_of(s) {
            return Ok(s.rt.block_on(hub.holding()).unwrap_or_default());
        }
        let candidate = s.headset_addr.lock().map_err(|e| e.to_string())?.clone();
        if let Some(address) = candidate {
            if matches!(s.rt.block_on(s.audio.audio_state(&address)), Ok(AudioState::Connected)) {
                return Ok(address);
            }
        }
        Ok(String::new())
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

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;
