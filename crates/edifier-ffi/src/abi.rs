//! C ABI. `unsafe` 函数的句柄必须来自对应 `_new` 且尚未 free, 字符串为 UTF-8 或 NULL.
#![allow(clippy::missing_safety_doc)]

use std::os::raw::{c_char, c_int};
use std::ptr;

use edifier_group::{derive_group, message::parse_nonce, open, seal, HandoffMachine};
use edifier_protocol::FrameDecoder;

use crate::action_json::{actions_json, parse_mac};
use crate::command_json::encode_command_json;
use crate::cstr::{fail, from_ptr, last_error_ptr, to_raw};
use crate::notify_json::{decoder_push_hex, parse_frame_hex};
use crate::profiles::{profiles_json, readout_json, settings_parse_json};

const VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), "\0");

fn read_str<'a>(ptr: *const c_char) -> Result<&'a str, String> {
    from_ptr(ptr).map_err(str::to_string)
}

fn ok_json<T: serde::Serialize>(value: T) -> *mut c_char {
    match serde_json::to_string(&value) {
        Ok(text) => to_raw(text),
        Err(err) => fail(err.to_string()),
    }
}

fn ok_text(result: Result<String, String>) -> *mut c_char {
    match result {
        Ok(text) => to_raw(text),
        Err(err) => fail(err),
    }
}

#[no_mangle]
pub extern "C" fn edifier_version() -> *const c_char {
    VERSION.as_ptr() as *const c_char
}

#[no_mangle]
pub extern "C" fn edifier_last_error() -> *const c_char {
    last_error_ptr()
}

#[no_mangle]
pub unsafe extern "C" fn edifier_string_free(s: *mut c_char) {
    unsafe { crate::cstr::free(s) };
}

#[no_mangle]
pub extern "C" fn edifier_command_encode(command_json: *const c_char) -> *mut c_char {
    ok_text((|| {
        let encoded = encode_command_json(read_str(command_json)?)?;
        serde_json::to_string(&encoded).map_err(|e| e.to_string())
    })())
}

#[no_mangle]
pub extern "C" fn edifier_frame_parse(frame_hex: *const c_char) -> *mut c_char {
    match read_str(frame_hex).and_then(parse_frame_hex) {
        Ok(parsed) => ok_json(parsed),
        Err(err) => fail(err),
    }
}

#[no_mangle]
pub extern "C" fn edifier_profiles_json() -> *mut c_char {
    ok_text(profiles_json())
}

#[no_mangle]
pub extern "C" fn edifier_readout_plan(profile_key: *const c_char) -> *mut c_char {
    ok_text(read_str(profile_key).and_then(readout_json))
}

#[no_mangle]
pub extern "C" fn edifier_settings_parse(settings_json: *const c_char) -> *mut c_char {
    ok_text(read_str(settings_json).and_then(settings_parse_json))
}

#[no_mangle]
pub extern "C" fn edifier_group_id(passphrase: *const c_char) -> *mut c_char {
    match read_str(passphrase) {
        Ok(pass) => {
            let (gid, _) = derive_group(pass);
            to_raw(gid.to_hex())
        }
        Err(err) => fail(err),
    }
}

#[no_mangle]
pub extern "C" fn edifier_envelope_seal(
    passphrase: *const c_char,
    ts_ms: u64,
    message_json: *const c_char,
) -> *mut c_char {
    ok_text((|| {
        let pass = read_str(passphrase)?;
        let body = serde_json::from_str(read_str(message_json)?).map_err(|e| e.to_string())?;
        let (gid, key) = derive_group(pass);
        let env = seal(&key, gid, ts_ms, body).map_err(|e| format!("{e:?}"))?;
        serde_json::to_string(&env).map_err(|e| e.to_string())
    })())
}

#[no_mangle]
pub extern "C" fn edifier_envelope_open(
    passphrase: *const c_char,
    now_ms: u64,
    envelope_json: *const c_char,
) -> *mut c_char {
    ok_text((|| {
        let pass = read_str(passphrase)?;
        let env = serde_json::from_str(read_str(envelope_json)?).map_err(|e| e.to_string())?;
        let (gid, key) = derive_group(pass);
        let body = open(&key, gid, now_ms, &env).map_err(|e| format!("{e:?}"))?;
        serde_json::to_string(&body).map_err(|e| e.to_string())
    })())
}

#[no_mangle]
pub extern "C" fn edifier_decoder_new() -> *mut FrameDecoder {
    Box::into_raw(Box::new(FrameDecoder::new()))
}

#[no_mangle]
pub unsafe extern "C" fn edifier_decoder_free(decoder: *mut FrameDecoder) {
    if !decoder.is_null() {
        drop(unsafe { Box::from_raw(decoder) });
    }
}

#[no_mangle]
pub unsafe extern "C" fn edifier_decoder_push_hex(
    decoder: *mut FrameDecoder,
    hex: *const c_char,
) -> *mut c_char {
    let Some(decoder) = (unsafe { decoder.as_mut() }) else {
        return fail("decoder 为空");
    };
    match read_str(hex).and_then(|s| decoder_push_hex(decoder, s)) {
        Ok(frames) => ok_json(frames),
        Err(err) => fail(err),
    }
}

#[no_mangle]
pub extern "C" fn edifier_handoff_new(local_id: *const c_char) -> *mut HandoffMachine {
    match read_str(local_id) {
        Ok(id) => Box::into_raw(Box::new(HandoffMachine::new(id))),
        Err(err) => {
            crate::cstr::set_error(err);
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn edifier_handoff_free(machine: *mut HandoffMachine) {
    if !machine.is_null() {
        drop(unsafe { Box::from_raw(machine) });
    }
}

#[no_mangle]
pub unsafe extern "C" fn edifier_handoff_set_has_audio(
    machine: *mut HandoffMachine,
    has_audio: c_int,
) {
    if let Some(m) = unsafe { machine.as_mut() } {
        m.has_audio = has_audio != 0;
    }
}

#[no_mangle]
pub unsafe extern "C" fn edifier_handoff_set_can_control(
    machine: *mut HandoffMachine,
    can_control: c_int,
) {
    if let Some(m) = unsafe { machine.as_mut() } {
        m.can_control_headset = can_control != 0;
    }
}

#[no_mangle]
pub unsafe extern "C" fn edifier_handoff_claim(
    machine: *mut HandoffMachine,
    mac: *const c_char,
    nonce_hex: *const c_char,
    now_ms: u64,
) -> *mut c_char {
    let Some(m) = (unsafe { machine.as_mut() }) else {
        return fail("handoff 为空");
    };
    ok_text((|| {
        let mac = parse_mac(read_str(mac)?)?;
        let nonce = parse_nonce(read_str(nonce_hex)?).ok_or_else(|| "nonce 必须是 16 字节 hex".to_string())?;
        actions_json(m.claim(mac, nonce, now_ms))
    })())
}

#[no_mangle]
pub unsafe extern "C" fn edifier_handoff_on_message(
    machine: *mut HandoffMachine,
    message_json: *const c_char,
    now_ms: u64,
) -> *mut c_char {
    let Some(m) = (unsafe { machine.as_mut() }) else {
        return fail("handoff 为空");
    };
    ok_text((|| {
        let msg = serde_json::from_str(read_str(message_json)?).map_err(|e| e.to_string())?;
        actions_json(m.on_message(&msg, now_ms))
    })())
}

#[no_mangle]
pub unsafe extern "C" fn edifier_handoff_tick(
    machine: *mut HandoffMachine,
    now_ms: u64,
) -> *mut c_char {
    let Some(m) = (unsafe { machine.as_mut() }) else {
        return fail("handoff 为空");
    };
    ok_text(actions_json(m.tick(now_ms)))
}

#[no_mangle]
pub unsafe extern "C" fn edifier_handoff_on_audio_connected(
    machine: *mut HandoffMachine,
) -> *mut c_char {
    let Some(m) = (unsafe { machine.as_mut() }) else {
        return fail("handoff 为空");
    };
    ok_text(actions_json(m.on_audio_connected()))
}

#[no_mangle]
pub unsafe extern "C" fn edifier_handoff_on_audio_disconnected(
    machine: *mut HandoffMachine,
) -> *mut c_char {
    let Some(m) = (unsafe { machine.as_mut() }) else {
        return fail("handoff 为空");
    };
    ok_text(actions_json(m.on_audio_disconnected()))
}

#[no_mangle]
pub unsafe extern "C" fn edifier_handoff_on_audio_failed(
    machine: *mut HandoffMachine,
    reason: *const c_char,
) -> *mut c_char {
    let Some(m) = (unsafe { machine.as_mut() }) else {
        return fail("handoff 为空");
    };
    ok_text(read_str(reason).and_then(|r| actions_json(m.on_audio_failed(r))))
}
