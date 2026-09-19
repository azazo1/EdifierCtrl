use std::ffi::{CStr, CString};

use super::*;
use edifier_runtime::HeadsetTransport;

fn text(pointer: *mut c_char) -> String {
    assert!(!pointer.is_null());
    let text = unsafe { CStr::from_ptr(pointer) }.to_string_lossy().into_owned();
    unsafe { crate::edifier_string_free(pointer) };
    text
}

#[test]
fn holding_normalizes_address_and_requires_actual_audio() {
    let session = edifier_session_new(std::ptr::null());
    let s = unsafe { &*session };
    let address = CString::new("aa-bb-cc-dd-ee-ff").unwrap();
    assert_eq!(edifier_session_set_holding(session, address.as_ptr()), 0);
    assert_eq!(text(edifier_session_holding(session)), "");
    s.rt.block_on(s.audio.connect_audio("AA:BB:CC:DD:EE:FF")).unwrap();
    assert_eq!(text(edifier_session_holding(session)), "AA:BB:CC:DD:EE:FF");
    let invalid = CString::new("not-a-mac").unwrap();
    assert_eq!(edifier_session_set_holding(session, invalid.as_ptr()), -1);
    assert_eq!(text(edifier_session_holding(session)), "AA:BB:CC:DD:EE:FF");
    s.rt.block_on(s.audio.disconnect_audio("AA:BB:CC:DD:EE:FF")).unwrap();
    assert_eq!(text(edifier_session_holding(session)), "");
    unsafe { edifier_session_free(session) };
}

#[test]
fn group_leave_is_idempotent_and_empty_join_has_no_side_effects() {
    let session = edifier_session_new(std::ptr::null());
    let empty = CString::new(" \t").unwrap();
    assert_eq!(edifier_session_group_join(session, empty.as_ptr()), -1);
    assert_eq!(edifier_session_group_leave(session), 0);
    assert_eq!(edifier_session_group_leave(session), 0);
    let s = unsafe { &*session };
    assert!(s.group.lock().unwrap().is_none());
    assert!(s.group_task.lock().unwrap().is_none());
    assert!(s.group_events.lock().unwrap().is_none());
    unsafe { edifier_session_destroy(session) };
}

#[test]
fn destroy_closes_control_transport_and_keeps_system_audio() {
    let session = edifier_session_new(std::ptr::null());
    let address = CString::new("11:22:33:44:55:66").unwrap();
    let kind = CString::new("rfcomm").unwrap();
    assert_eq!(edifier_session_connect(session, address.as_ptr(), kind.as_ptr()), 0);
    let s = unsafe { &*session };
    let host = s.host.clone();
    let audio = s.audio.clone();
    s.rt.block_on(audio.connect_audio("11:22:33:44:55:66")).unwrap();
    unsafe { edifier_session_destroy(session) };
    let rt = Runtime::new().unwrap();
    rt.block_on(async {
        assert!(!host.connected().await);
        assert!(host.transport().write(&[0xAA]).await.is_err());
        assert_eq!(audio.audio_state("11:22:33:44:55:66").await.unwrap(), AudioState::Connected);
    });
}
