use std::cell::RefCell;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::ptr;

thread_local! {
    static LAST_ERROR: RefCell<Option<CString>> = const { RefCell::new(None) };
}

pub fn set_error(msg: impl AsRef<str>) {
    let msg = msg.as_ref();
    let cstr = CString::new(msg).unwrap_or_else(|_| CString::new("error").unwrap());
    LAST_ERROR.with(|slot| *slot.borrow_mut() = Some(cstr));
}

pub fn last_error_ptr() -> *const c_char {
    LAST_ERROR.with(|slot| match slot.borrow().as_ref() {
        Some(s) => s.as_ptr(),
        None => ptr::null(),
    })
}

pub fn to_raw(s: String) -> *mut c_char {
    match CString::new(s) {
        Ok(cstr) => cstr.into_raw(),
        Err(_) => {
            set_error("结果包含 NUL");
            ptr::null_mut()
        }
    }
}

pub fn from_ptr<'a>(ptr: *const c_char) -> Result<&'a str, &'static str> {
    if ptr.is_null() {
        return Err("空指针");
    }
    unsafe { CStr::from_ptr(ptr) }
        .to_str()
        .map_err(|_| "非法 UTF-8")
}

pub fn fail(msg: impl AsRef<str>) -> *mut c_char {
    set_error(msg);
    ptr::null_mut()
}

/// 释放 `to_raw` 分配的字符串.
///
/// # Safety
/// `s` 必须是本库返回的堆字符串或 NULL.
pub unsafe fn free(s: *mut c_char) {
    if !s.is_null() {
        drop(unsafe { CString::from_raw(s) });
    }
}
