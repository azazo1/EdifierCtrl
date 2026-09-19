//! Android JNI, 对应 `dev.edifierctrl.app.EdifierNative`.

use std::ffi::CStr;
use std::os::raw::{c_char, c_void};

use jni::objects::{JClass, JString};
use jni::sys::{jint, jlong, jstring, JNI_VERSION_1_6};
use jni::JNIEnv;
use jni::JavaVM;

use crate::{
    edifier_command_encode, edifier_frame_parse, edifier_last_error, edifier_profiles_json,
    edifier_session_connect, edifier_session_disconnect, edifier_session_free,
    edifier_session_group_claim,
    edifier_session_group_claim_peer, edifier_session_group_id_hex, edifier_session_group_join,
    edifier_session_group_leave,
    edifier_session_group_peers, edifier_session_holding, edifier_session_new,
    edifier_session_poll_event, edifier_session_set_holding,
    edifier_session_readout, edifier_session_scan, edifier_session_send_json, edifier_string_free,
    edifier_version,
    FfiSession,
};

fn throw(env: &mut JNIEnv, msg: &str) {
    let _ = env.throw_new("java/lang/IllegalStateException", msg);
}

fn last_error_text() -> String {
    let ptr = edifier_last_error();
    if ptr.is_null() {
        return "未知 FFI 错误".into();
    }
    unsafe { CStr::from_ptr(ptr) }
        .to_string_lossy()
        .into_owned()
}

fn req_str(env: &mut JNIEnv, value: JString) -> Result<String, String> {
    env.get_string(&value)
        .map(|s| s.into())
        .map_err(|e| e.to_string())
}

fn to_jstring(env: &mut JNIEnv, text: &str) -> jstring {
    match env.new_string(text) {
        Ok(s) => s.into_raw(),
        Err(err) => {
            throw(env, &err.to_string());
            std::ptr::null_mut()
        }
    }
}

fn take_c_string(ptr: *mut c_char) -> Result<String, String> {
    if ptr.is_null() {
        return Err(last_error_text());
    }
    let text = unsafe { CStr::from_ptr(ptr) }
        .to_string_lossy()
        .into_owned();
    unsafe { edifier_string_free(ptr) };
    Ok(text)
}

fn owned_cstr(text: &str) -> Result<std::ffi::CString, String> {
    std::ffi::CString::new(text).map_err(|_| "字符串含 NUL".into())
}

fn session_ptr(handle: jlong) -> *mut FfiSession {
    handle as *mut FfiSession
}

#[no_mangle]
pub extern "system" fn Java_dev_edifierctrl_app_EdifierNative_version(
    mut env: JNIEnv,
    _class: JClass,
) -> jstring {
    let ptr = edifier_version();
    let text = unsafe { CStr::from_ptr(ptr) }.to_string_lossy();
    to_jstring(&mut env, &text)
}

#[no_mangle]
pub extern "system" fn Java_dev_edifierctrl_app_EdifierNative_lastError(
    mut env: JNIEnv,
    _class: JClass,
) -> jstring {
    to_jstring(&mut env, &last_error_text())
}

#[no_mangle]
pub extern "system" fn Java_dev_edifierctrl_app_EdifierNative_commandEncode(
    mut env: JNIEnv,
    _class: JClass,
    command_json: JString,
) -> jstring {
    match req_str(&mut env, command_json).and_then(|json| {
        let c = owned_cstr(&json)?;
        take_c_string(edifier_command_encode(c.as_ptr()))
    }) {
        Ok(text) => to_jstring(&mut env, &text),
        Err(err) => {
            throw(&mut env, &err);
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_dev_edifierctrl_app_EdifierNative_frameParse(
    mut env: JNIEnv,
    _class: JClass,
    frame_hex: JString,
) -> jstring {
    match req_str(&mut env, frame_hex).and_then(|hex| {
        let c = owned_cstr(&hex)?;
        take_c_string(edifier_frame_parse(c.as_ptr()))
    }) {
        Ok(text) => to_jstring(&mut env, &text),
        Err(err) => {
            throw(&mut env, &err);
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_dev_edifierctrl_app_EdifierNative_profilesJson(
    mut env: JNIEnv,
    _class: JClass,
) -> jstring {
    match take_c_string(edifier_profiles_json()) {
        Ok(text) => to_jstring(&mut env, &text),
        Err(err) => {
            throw(&mut env, &err);
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_dev_edifierctrl_app_EdifierNative_sessionNew(
    mut env: JNIEnv,
    _class: JClass,
    local_id: JString,
) -> jlong {
    match req_str(&mut env, local_id).and_then(|id| {
        let c = owned_cstr(&id)?;
        let ptr = edifier_session_new(c.as_ptr());
        if ptr.is_null() {
            Err(last_error_text())
        } else {
            Ok(ptr as jlong)
        }
    }) {
        Ok(handle) => handle,
        Err(err) => {
            throw(&mut env, &err);
            0
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_dev_edifierctrl_app_EdifierNative_sessionFree(
    _env: JNIEnv,
    _class: JClass,
    session: jlong,
) {
    unsafe { edifier_session_free(session_ptr(session)) };
}

#[no_mangle]
pub extern "system" fn Java_dev_edifierctrl_app_EdifierNative_sessionScan(
    mut env: JNIEnv,
    _class: JClass,
    session: jlong,
    kind: JString,
) -> jstring {
    match req_str(&mut env, kind).and_then(|kind| {
        let c = owned_cstr(&kind)?;
        take_c_string(edifier_session_scan(session_ptr(session), c.as_ptr()))
    }) {
        Ok(text) => to_jstring(&mut env, &text),
        Err(err) => {
            throw(&mut env, &err);
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_dev_edifierctrl_app_EdifierNative_sessionConnect(
    mut env: JNIEnv,
    _class: JClass,
    session: jlong,
    address: JString,
    kind: JString,
) -> jint {
    match (req_str(&mut env, address), req_str(&mut env, kind)) {
        (Ok(address), Ok(kind)) => match (owned_cstr(&address), owned_cstr(&kind)) {
            (Ok(a), Ok(k)) => {
                edifier_session_connect(session_ptr(session), a.as_ptr(), k.as_ptr())
            }
            (Err(err), _) | (_, Err(err)) => {
                throw(&mut env, &err);
                -1
            }
        },
        (Err(err), _) | (_, Err(err)) => {
            throw(&mut env, &err);
            -1
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_dev_edifierctrl_app_EdifierNative_sessionDisconnect(
    _env: JNIEnv,
    _class: JClass,
    session: jlong,
) -> jint {
    edifier_session_disconnect(session_ptr(session))
}

#[no_mangle]
pub extern "system" fn Java_dev_edifierctrl_app_EdifierNative_sessionReadout(
    mut env: JNIEnv,
    _class: JClass,
    session: jlong,
    profile_key: JString,
) -> jint {
    match req_str(&mut env, profile_key).and_then(|k| owned_cstr(&k)) {
        Ok(c) => edifier_session_readout(session_ptr(session), c.as_ptr()),
        Err(err) => {
            throw(&mut env, &err);
            -1
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_dev_edifierctrl_app_EdifierNative_sessionSendJson(
    mut env: JNIEnv,
    _class: JClass,
    session: jlong,
    command_json: JString,
) -> jint {
    match req_str(&mut env, command_json).and_then(|json| owned_cstr(&json)) {
        Ok(c) => edifier_session_send_json(session_ptr(session), c.as_ptr()),
        Err(err) => {
            throw(&mut env, &err);
            -1
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_dev_edifierctrl_app_EdifierNative_sessionPollEvent(
    mut env: JNIEnv,
    _class: JClass,
    session: jlong,
) -> jstring {
    match take_c_string(edifier_session_poll_event(session_ptr(session))) {
        Ok(text) => to_jstring(&mut env, &text),
        Err(err) => {
            throw(&mut env, &err);
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_dev_edifierctrl_app_EdifierNative_sessionGroupJoin(
    mut env: JNIEnv,
    _class: JClass,
    session: jlong,
    passphrase: JString,
) -> jint {
    match req_str(&mut env, passphrase).and_then(|p| owned_cstr(&p)) {
        Ok(c) => edifier_session_group_join(session_ptr(session), c.as_ptr()),
        Err(err) => {
            throw(&mut env, &err);
            -1
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_dev_edifierctrl_app_EdifierNative_sessionGroupLeave(
    _env: JNIEnv,
    _class: JClass,
    session: jlong,
) -> jint {
    edifier_session_group_leave(session_ptr(session))
}

#[no_mangle]
pub extern "system" fn Java_dev_edifierctrl_app_EdifierNative_sessionGroupPeers(
    mut env: JNIEnv,
    _class: JClass,
    session: jlong,
) -> jstring {
    match take_c_string(edifier_session_group_peers(session_ptr(session))) {
        Ok(text) => to_jstring(&mut env, &text),
        Err(err) => {
            throw(&mut env, &err);
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_dev_edifierctrl_app_EdifierNative_sessionGroupClaim(
    mut env: JNIEnv,
    _class: JClass,
    session: jlong,
    mac: JString,
) -> jint {
    match req_str(&mut env, mac).and_then(|m| owned_cstr(&m)) {
        Ok(c) => edifier_session_group_claim(session_ptr(session), c.as_ptr()),
        Err(err) => {
            throw(&mut env, &err);
            -1
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_dev_edifierctrl_app_EdifierNative_sessionGroupClaimPeer(
    mut env: JNIEnv,
    _class: JClass,
    session: jlong,
    peer_id: JString,
) -> jint {
    match req_str(&mut env, peer_id).and_then(|id| owned_cstr(&id)) {
        Ok(c) => edifier_session_group_claim_peer(session_ptr(session), c.as_ptr()),
        Err(err) => {
            throw(&mut env, &err);
            -1
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_dev_edifierctrl_app_EdifierNative_sessionGroupIdHex(
    mut env: JNIEnv,
    _class: JClass,
    session: jlong,
) -> jstring {
    match take_c_string(edifier_session_group_id_hex(session_ptr(session))) {
        Ok(text) => to_jstring(&mut env, &text),
        Err(err) => {
            throw(&mut env, &err);
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_dev_edifierctrl_app_EdifierNative_sessionSetHolding(
    mut env: JNIEnv,
    _class: JClass,
    session: jlong,
    mac: JString,
) -> jint {
    match req_str(&mut env, mac).and_then(|m| owned_cstr(&m)) {
        Ok(c) => edifier_session_set_holding(session_ptr(session), c.as_ptr()),
        Err(err) => {
            throw(&mut env, &err);
            -1
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_dev_edifierctrl_app_EdifierNative_sessionHolding(
    mut env: JNIEnv,
    _class: JClass,
    session: jlong,
) -> jstring {
    match take_c_string(edifier_session_holding(session_ptr(session))) {
        Ok(text) => to_jstring(&mut env, &text),
        Err(err) => {
            throw(&mut env, &err);
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "system" fn JNI_OnLoad(vm: *mut jni::sys::JavaVM, _reserved: *mut c_void) -> jint {
    match unsafe { JavaVM::from_raw(vm) } {
        Ok(vm) => {
            if let Err(err) = edifier_bt_android::bind_jvm(vm) {
                tracing::warn!(target: "edifier_ffi", %err, "绑定 BluetoothBridge 失败");
            }
        }
        Err(err) => {
            tracing::warn!(target: "edifier_ffi", %err, "JNI_OnLoad 无法拿到 JavaVM");
        }
    }
    JNI_VERSION_1_6
}
