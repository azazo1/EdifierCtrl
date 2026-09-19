use edifier_runtime::TransportError;
use windows::core::GUID;
use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};

use crate::addr;

pub const RFCOMM_GUID: GUID = GUID::from_u128(0xedf00000_edfe_dfed_fedf_edfedfedfedf);

/// WinRT 对象在 MTA 下跨线程. 所有调用都先 `init_mta`.
pub struct Mta<T>(pub T);

unsafe impl<T> Send for Mta<T> {}
unsafe impl<T> Sync for Mta<T> {}

pub fn init_mta() {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }
}

pub fn spawn_mta<F>(name: &str, f: F) -> Result<std::thread::JoinHandle<()>, TransportError>
where
    F: FnOnce() + 'static,
{
    struct Packet {
        ptr: *mut (dyn FnOnce() + 'static),
    }
    unsafe impl Send for Packet {}
    impl Packet {
        fn run(self) {
            let f = unsafe { Box::from_raw(self.ptr) };
            f();
        }
    }
    let packet = Packet {
        ptr: Box::into_raw(Box::new(f) as Box<dyn FnOnce() + 'static>),
    };
    std::thread::Builder::new()
        .name(name.into())
        .spawn(move || {
            init_mta();
            packet.run();
        })
        .map_err(|e| TransportError::Connect(e.to_string()))
}

pub fn win_err(err: windows::core::Error) -> TransportError {
    TransportError::Connect(err.to_string())
}

pub async fn blocking<T, F>(f: F) -> Result<T, TransportError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, TransportError> + Send + 'static,
{
    tokio::task::spawn_blocking(move || {
        init_mta();
        f()
    })
    .await
    .map_err(|e| TransportError::Connect(format!("后台任务失败: {e}")))?
}

pub fn parse_addr(s: &str) -> Result<u64, TransportError> {
    addr::parse_u64(s)
}

pub fn format_addr(addr: u64) -> String {
    addr::from_u64(addr)
}

pub fn guid_text(guid: GUID) -> String {
    format!("{guid:?}")
}
