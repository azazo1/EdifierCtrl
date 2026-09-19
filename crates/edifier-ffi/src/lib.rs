//! 稳定 C ABI. 头文件见 `include/edifier.h`.

mod abi;
mod action_json;
mod command_json;
mod cstr;
mod event_json;
mod logging;
mod notify_json;
#[cfg(not(test))]
mod platform;
#[cfg(test)]
#[path = "test_platform.rs"]
mod platform;
mod profiles;
mod session;

#[cfg(target_os = "android")]
mod jni_bridge;

pub use abi::*;
pub use logging::{edifier_log_install, edifier_log_set_level, EdifierLogCallback};
pub use session::*;

#[cfg(test)]
mod tests {
    use std::ffi::{CStr, CString};
    use std::os::raw::c_char;

    use super::*;

    fn cstr(s: &str) -> CString {
        CString::new(s).unwrap()
    }

    fn take(ptr: *mut c_char) -> String {
        assert!(!ptr.is_null(), "{}", last_error());
        unsafe {
            let text = CStr::from_ptr(ptr).to_str().unwrap().to_string();
            edifier_string_free(ptr);
            text
        }
    }

    fn last_error() -> String {
        let p = edifier_last_error();
        if p.is_null() {
            return String::new();
        }
        unsafe { CStr::from_ptr(p).to_str().unwrap().to_string() }
    }

    #[test]
    fn version_nonempty() {
        let v = unsafe { CStr::from_ptr(edifier_version()) };
        assert_eq!(v.to_str().unwrap(), "0.1.0");
    }

    #[test]
    fn encode_noise_normal() {
        let input = cstr(r#"{"op":"set_noise_mode","mode":"normal"}"#);
        let out = take(edifier_command_encode(input.as_ptr()));
        assert!(out.contains("AA02C1012187"), "{out}");
        assert!(out.contains("\"destructive\":false"));
    }

    #[test]
    fn parse_battery_frame() {
        let input = cstr("BB02D04D21F3");
        let out = take(edifier_frame_parse(input.as_ptr()));
        assert!(out.contains("\"kind\":\"battery\""));
        assert!(out.contains("\"percent\":77"));
    }

    #[test]
    fn profiles_include_w820nb() {
        let out = take(edifier_profiles_json());
        assert!(out.contains("w820nb"));
        assert!(out.contains("noise"));
    }

    #[test]
    fn readout_skips_ldac_on_w200() {
        let key = cstr("w200btplus");
        let out = take(edifier_readout_plan(key.as_ptr()));
        assert!(!out.contains("query_ldac"));
        assert!(out.contains("query_battery"));
    }

    #[test]
    fn decoder_split_push() {
        let dec = edifier_decoder_new();
        let a = cstr("BB02D0");
        let first = unsafe { edifier_decoder_push_hex(dec, a.as_ptr()) };
        let first_text = take(first);
        assert_eq!(first_text, "[]");
        let b = cstr("4D21F3");
        let second = unsafe { edifier_decoder_push_hex(dec, b.as_ptr()) };
        let second_text = take(second);
        assert!(second_text.contains("battery"));
        unsafe { edifier_decoder_free(dec) };
    }

    #[test]
    fn handoff_claim_emits_request() {
        let id = cstr("host-b");
        let machine = edifier_handoff_new(id.as_ptr());
        let mac = cstr("11:22:33:44:55:66");
        let nonce = cstr("07070707070707070707070707070707");
        let actions = unsafe { take(edifier_handoff_claim(machine, mac.as_ptr(), nonce.as_ptr(), 0)) };
        assert!(actions.contains("handoff_request"));
        assert!(actions.contains("requesting"));
        unsafe { edifier_handoff_free(machine) };
    }

    #[test]
    fn session_polls_empty() {
        let id = cstr("ui-test");
        let session = edifier_session_new(id.as_ptr());
        assert!(!session.is_null(), "{}", last_error());
        let ev = take(edifier_session_poll_event(session));
        assert!(ev.contains("empty"), "{ev}");
        unsafe { edifier_session_free(session) };
    }

    #[test]
    fn readout_without_connect_fails() {
        let id = cstr("ui-test");
        let session = edifier_session_new(id.as_ptr());
        assert!(!session.is_null(), "{}", last_error());
        let key = cstr("basedevice");
        let rc = edifier_session_readout(session, key.as_ptr());
        assert_eq!(rc, -1);
        unsafe { edifier_session_free(session) };
    }

    #[test]
    fn set_holding_does_not_invent_audio_connection() {
        let id = cstr("ui-test");
        let session = edifier_session_new(id.as_ptr());
        assert!(!session.is_null(), "{}", last_error());
        let mac = cstr("AA:BB:CC:DD:EE:FF");
        assert_eq!(edifier_session_set_holding(session, mac.as_ptr()), 0);
        let got = take(edifier_session_holding(session));
        assert_eq!(got, "");
        unsafe { edifier_session_free(session) };
    }

    #[test]
    fn claim_peer_without_group_fails() {
        let id = cstr("ui-test");
        let session = edifier_session_new(id.as_ptr());
        assert!(!session.is_null(), "{}", last_error());
        let peer = cstr("ghost");
        let rc = edifier_session_group_claim_peer(session, peer.as_ptr());
        assert_eq!(rc, -1);
        unsafe { edifier_session_free(session) };
    }

    #[test]
    fn envelope_roundtrip() {
        let pass = cstr("lan");
        let msg = cstr(r#"{"type":"handoff_busy","nonce":"aa"}"#);
        let sealed = take(edifier_envelope_seal(pass.as_ptr(), 1000, msg.as_ptr()));
        let env = cstr(&sealed);
        let opened = take(edifier_envelope_open(pass.as_ptr(), 1000, env.as_ptr()));
        assert!(opened.contains("handoff_busy"));
    }
}
