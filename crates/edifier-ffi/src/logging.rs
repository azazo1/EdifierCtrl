//! 将核心诊断交给原生宿主, 由宿主统一处理时间戳, 文件轮转和退出刷盘.

use std::cell::Cell;
use std::ffi::{c_char, c_int, CString};
use std::fmt::Write;
use std::sync::OnceLock;

use tracing::field::{Field, Visit};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::layer::Context;
use tracing_subscriber::{prelude::*, reload, EnvFilter, Layer, Registry};

use crate::cstr::set_error;

pub type EdifierLogCallback = unsafe extern "C" fn(c_int, *const c_char, *const c_char, c_int);
static CONTROL: OnceLock<Result<reload::Handle<EnvFilter, Registry>, String>> = OnceLock::new();
thread_local! { static IN_CALLBACK: Cell<bool> = const { Cell::new(false) }; }

/// 首次成功调用注册进程级宿主日志, 必须在创建会话前调用.
///
/// # Safety
/// 回调必须在进程余下生命周期内有效, 支持并发调用, 不抛异常或展开栈.
/// 字符串仅在回调期间有效. flush 非零时应在回调返回前完成刷盘.
#[no_mangle]
pub unsafe extern "C" fn edifier_log_install(callback: Option<EdifierLogCallback>) -> c_int {
    let Some(callback) = callback else {
        set_error("宿主日志回调不能为空");
        return -1;
    };
    match CONTROL.get_or_init(|| {
        let (filter, control) = reload::Layer::new(filter_for(Level::INFO));
        tracing_subscriber::registry().with(filter).with(HostLogLayer(callback))
            .try_init().map_err(|err| format!("初始化核心日志失败: {err}"))?;
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |panic| {
            // 不经过 tracing 过滤, panic 必须立即记录并同步刷盘.
            forward(callback, 1, "edifier_ffi::panic", &format!("Rust panic: {panic}"), true);
            previous(panic);
        }));
        Ok(control)
    }) {
        Ok(_) => 0,
        Err(error) => { set_error(error.clone()); -1 }
    }
}

/// 动态切换缺省级别. RUST_LOG 中显式指定的 target 指令仍然有效.
#[no_mangle]
pub extern "C" fn edifier_log_set_level(level: c_int) -> c_int {
    let level = match level {
        1 => Level::ERROR,
        2 => Level::WARN,
        3 => Level::INFO,
        4 => Level::DEBUG,
        5 => Level::TRACE,
        _ => { set_error("核心日志级别必须为 1...5"); return -1; }
    };
    let Some(Ok(control)) = CONTROL.get() else {
        set_error("核心日志尚未初始化");
        return -1;
    };
    match control.reload(filter_for(level)) {
        Ok(()) => 0,
        Err(err) => { set_error(format!("切换核心日志级别失败: {err}")); -1 }
    }
}

fn filter_for(level: Level) -> EnvFilter {
    EnvFilter::builder().with_default_directive(level.into()).from_env_lossy()
}

struct HostLogLayer(EdifierLogCallback);

impl<S: Subscriber> Layer<S> for HostLogLayer {
    fn on_event(&self, event: &Event<'_>, _: Context<'_, S>) {
        let mut fields = EventFields::default();
        event.record(&mut fields);
        if !fields.details.is_empty() {
            if !fields.message.is_empty() { fields.message.push_str(". "); }
            fields.message.push_str(&fields.details);
        }
        let level = match *event.metadata().level() {
            Level::ERROR => 1,
            Level::WARN => 2,
            Level::INFO => 3,
            Level::DEBUG => 4,
            Level::TRACE => 5,
        };
        forward(self.0, level, event.metadata().target(), &fields.message, false);
    }
}

#[derive(Default)]
struct EventFields {
    message: String,
    details: String,
}

impl Visit for EventFields {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            let _ = write!(self.message, "{value:?}");
        } else {
            if !self.details.is_empty() { self.details.push_str(", "); }
            let _ = write!(self.details, "{}={value:?}", field.name());
        }
    }
}

fn forward(callback: EdifierLogCallback, level: c_int, target: &str, message: &str, flush: bool) {
    IN_CALLBACK.with(|active| {
        if active.replace(true) { return; }
        struct Reset<'a>(&'a Cell<bool>);
        impl Drop for Reset<'_> { fn drop(&mut self) { self.0.set(false); } }
        let _reset = Reset(active);
        // C 字符串不能含 NUL, 以可见转义保留内容, 不截断诊断.
        let target = CString::new(target.replace('\0', "\\0")).expect("已转义 NUL");
        let message = CString::new(message.replace('\0', "\\0")).expect("已转义 NUL");
        unsafe { callback(level, target.as_ptr(), message.as_ptr(), c_int::from(flush)) };
    });
}

#[cfg(test)]
mod tests;
