use std::ffi::CStr;
use std::sync::Mutex;

use super::*;

#[derive(Debug, Clone)]
struct Record {
    level: c_int,
    target: String,
    message: String,
    flush: bool,
}
static RECORDS: Mutex<Vec<Record>> = Mutex::new(Vec::new());

unsafe extern "C" fn capture(level: c_int, target: *const c_char, message: *const c_char, flush: c_int) {
    RECORDS.lock().unwrap().push(Record {
        level,
        target: unsafe { CStr::from_ptr(target) }.to_string_lossy().into_owned(),
        message: unsafe { CStr::from_ptr(message) }.to_string_lossy().into_owned(),
        flush: flush != 0,
    });
    // 宿主在回调内意外记录 Rust 日志也不能递归或死锁.
    tracing::warn!(target: "edifier_log_test::recursive", "回调中的诊断");
}

#[test]
fn host_logging_preserves_worker_events_levels_and_flushes_panics() {
    // 订阅器和 panic hook 是进程级资源, 在独立测试进程验证, 不污染其它测试.
    if std::env::var_os("EDIFIER_LOG_TEST_CHILD").is_none() {
        let result = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "logging::tests::host_logging_preserves_worker_events_levels_and_flushes_panics", "--nocapture"])
            .env("EDIFIER_LOG_TEST_CHILD", "1")
            .env_remove("RUST_LOG")
            .output().unwrap();
        assert!(result.status.success(), "{}\n{}", String::from_utf8_lossy(&result.stdout), String::from_utf8_lossy(&result.stderr));
        return;
    }
    assert_eq!(unsafe { edifier_log_install(Some(capture)) }, 0);
    assert_eq!(unsafe { edifier_log_install(Some(capture)) }, 0);
    tracing::debug!(target: "edifier_log_test", "默认过滤详细日志");
    assert!(RECORDS.lock().unwrap().is_empty());
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        tokio::spawn(async {
            tracing::info!(target: "edifier_log_test", count = 7, "工作线程事件");
        }).await.unwrap();
    });
    assert_eq!(edifier_log_set_level(4), 0);
    tracing::debug!(target: "edifier_log_test", "运行时详细日志");
    tracing::trace!(target: "edifier_log_test", "尚未开启 trace");
    assert_eq!(edifier_log_set_level(5), 0);
    tracing::trace!(target: "edifier_log_test", "最详细日志");
    assert_eq!(edifier_log_set_level(3), 0);
    tracing::debug!(target: "edifier_log_test", "详细日志已关闭");
    forward(capture, 2, "edifier_log_test", "保留\0后面的内容", false);
    let panic = std::thread::spawn(|| panic!("诊断 panic 测试")).join();
    assert!(panic.is_err());
    let records = RECORDS.lock().unwrap().clone();
    assert_eq!(records.len(), 5, "{records:?}");
    assert_eq!(records.iter().map(|r| r.level).collect::<Vec<_>>(), [3, 4, 5, 2, 1]);
    assert_eq!(records[0].target, "edifier_log_test");
    assert!(records[0].message.contains("count=7"));
    assert!(records[3].message.contains("\\0"));
    assert_eq!(records[4].target, "edifier_ffi::panic");
    assert!(records[4].flush);
    assert!(records[..4].iter().all(|r| !r.flush));
}
