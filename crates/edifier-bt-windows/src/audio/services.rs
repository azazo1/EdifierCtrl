use std::collections::HashMap;
use std::time::{Duration, Instant};

use edifier_runtime::TransportError;
use tracing::{info, warn};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AudioService {
    A2dp,
    Handsfree,
    Headset,
}

pub(crate) trait ServiceControl {
    fn enabled(&self, address: &str) -> Result<Vec<AudioService>, TransportError>;
    fn set(&self, address: &str, service: AudioService, enable: bool) -> Result<(), TransportError>;
}

const RESTORE_SETTLE: Duration = Duration::from_secs(3);

#[derive(Default)]
pub(crate) struct ServiceState {
    restore: HashMap<String, Vec<AudioService>>,
    restored_at: HashMap<String, Instant>,
}

impl ServiceState {
    pub(crate) fn connect(
        &mut self,
        backend: &impl ServiceControl,
        address: &str,
    ) -> Result<(), TransportError> {
        if self.restored_at.remove(address).is_some_and(|at| at.elapsed() < RESTORE_SETTLE) {
            // 解除抑制刚恢复了服务, 交由调用方等待端点出现, 不立即再次关闭.
            return Ok(());
        }
        if self.restore.contains_key(address) {
            return self.restore_snapshot(backend, address);
        }
        let enabled = backend.enabled(address)?;
        if enabled.is_empty() {
            return set_services(backend, address, &[AudioService::A2dp], true);
        }
        // 已启用不代表已连接. 在关闭再开启之前保留原集合, 不额外启用免提服务.
        self.restore.insert(address.into(), enabled.clone());
        info!(target: "edifier_bt_windows", address, ?enabled, "重新启用原有音频服务以请求连接");
        for service in &enabled {
            if let Err(error) = set_service(backend, address, *service, false) {
                // 首个关闭失败立即恢复, 不继续移除其他服务驱动.
                return match self.restore_snapshot(backend, address) {
                    Ok(()) => Err(error),
                    Err(restore_error) => Err(TransportError::Connect(format!(
                        "音频重连失败: {error}; 恢复原服务失败: {restore_error}"
                    ))),
                };
            }
        }
        self.restore_snapshot(backend, address)
    }

    pub(crate) fn observed_connected(&mut self, address: &str) {
        self.restored_at.remove(address);
    }

    pub(crate) fn disconnect(
        &mut self,
        backend: &impl ServiceControl,
        address: &str,
    ) -> Result<(), TransportError> {
        self.restored_at.remove(address);
        let enabled = backend.enabled(address)?;
        let restore = self.restore.entry(address.into()).or_default();
        // 在首个副作用之前记录, 部分失败和再次释放都不能覆盖原快照.
        for service in &enabled {
            if !restore.contains(service) {
                restore.push(*service);
            }
        }
        set_services(backend, address, &enabled, false)
    }

    pub(crate) fn restore(
        &mut self,
        backend: &impl ServiceControl,
        address: &str,
    ) -> Result<(), TransportError> {
        self.restored_at.remove(address);
        let had_snapshot = self.restore.contains_key(address);
        self.restore_snapshot(backend, address)?;
        if had_snapshot {
            self.restored_at.insert(address.into(), Instant::now());
        }
        Ok(())
    }

    fn restore_snapshot(
        &mut self,
        backend: &impl ServiceControl,
        address: &str,
    ) -> Result<(), TransportError> {
        let Some(services) = self.restore.get(address).cloned() else {
            return Ok(());
        };
        set_services(backend, address, &services, true)?;
        self.restore.remove(address);
        Ok(())
    }
}

fn set_services(
    backend: &impl ServiceControl,
    address: &str,
    services: &[AudioService],
    enable: bool,
) -> Result<(), TransportError> {
    let mut failure = None;
    for service in services {
        if let Err(error) = set_service(backend, address, *service, enable) {
            failure = Some(error);
        }
    }
    match failure {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

fn set_service(
    backend: &impl ServiceControl,
    address: &str,
    service: AudioService,
    enable: bool,
) -> Result<(), TransportError> {
    if backend.enabled(address)?.contains(&service) == enable {
        return Ok(());
    }
    match backend.set(address, service, enable) {
        Ok(()) => Ok(()),
        Err(error) => {
            // 返回 87 或 E_INVALIDARG 也必须独立核验服务状态, 不能推断音频已连接.
            match backend.enabled(address) {
                Ok(enabled) if enabled.contains(&service) == enable => {
                    warn!(target: "edifier_bt_windows", address, ?service, enable, %error,
                        "服务设置返回错误, 但重新枚举已确认目标状态; 音频连接仍待端点确认");
                    Ok(())
                }
                Ok(_) => Err(error),
                Err(verify_error) => Err(TransportError::Connect(format!(
                    "{error}; 复核音频服务状态失败: {verify_error}"
                ))),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use super::*;

    #[derive(Default)]
    struct Backend {
        enabled: RefCell<Vec<AudioService>>,
        fail: RefCell<Option<(AudioService, bool)>>,
        calls: RefCell<Vec<(AudioService, bool)>>,
        fail_code: u32,
        fail_after_apply: bool,
    }

    impl ServiceControl for Backend {
        fn enabled(&self, _address: &str) -> Result<Vec<AudioService>, TransportError> {
            Ok(self.enabled.borrow().clone())
        }

        fn set(&self, _address: &str, service: AudioService, enable: bool) -> Result<(), TransportError> {
            self.calls.borrow_mut().push((service, enable));
            let fail = *self.fail.borrow() == Some((service, enable));
            if !fail || self.fail_after_apply {
                let mut enabled = self.enabled.borrow_mut();
                enabled.retain(|value| *value != service);
                if enable { enabled.push(service); }
            }
            if fail {
                Err(TransportError::Connect(format!("模拟服务失败 win32={}", self.fail_code)))
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn initial_connect_does_not_enable_handsfree() {
        let backend = Backend::default();
        ServiceState::default().connect(&backend, "a").unwrap();
        assert_eq!(*backend.enabled.borrow(), vec![AudioService::A2dp]);
    }

    #[test]
    fn first_connect_cycles_only_original_audio_services() {
        let backend = Backend::default();
        *backend.enabled.borrow_mut() = vec![AudioService::A2dp, AudioService::Handsfree];
        let mut state = ServiceState::default();
        state.connect(&backend, "a").unwrap();
        assert_eq!(*backend.calls.borrow(), vec![
            (AudioService::A2dp, false), (AudioService::Handsfree, false),
            (AudioService::A2dp, true), (AudioService::Handsfree, true),
        ]);
        assert_eq!(*backend.enabled.borrow(), vec![AudioService::A2dp, AudioService::Handsfree]);
        assert!(state.restore.is_empty());
    }

    #[test]
    fn restore_then_connect_does_not_cycle_services_again() {
        let backend = Backend::default();
        *backend.enabled.borrow_mut() = vec![AudioService::A2dp];
        let mut state = ServiceState::default();
        state.disconnect(&backend, "a").unwrap();
        state.restore(&backend, "a").unwrap();
        backend.calls.borrow_mut().clear();
        state.connect(&backend, "a").unwrap();
        assert!(backend.calls.borrow().is_empty());
        assert!(state.restored_at.is_empty());
    }

    #[test]
    fn expired_or_observed_restore_does_not_block_later_connect() {
        for already_observed in [false, true] {
            let backend = Backend::default();
            *backend.enabled.borrow_mut() = vec![AudioService::A2dp];
            let mut state = ServiceState::default();
            state.disconnect(&backend, "a").unwrap();
            state.restore(&backend, "a").unwrap();
            if already_observed {
                state.observed_connected("a");
            } else {
                state.restored_at.insert("a".into(), Instant::now() - RESTORE_SETTLE);
            }
            backend.calls.borrow_mut().clear();
            state.connect(&backend, "a").unwrap();
            assert_eq!(*backend.calls.borrow(), vec![
                (AudioService::A2dp, false), (AudioService::A2dp, true),
            ]);
        }
    }

    #[test]
    fn failed_cycle_restores_immediately_before_touching_remaining_services() {
        let backend = Backend::default();
        *backend.enabled.borrow_mut() = vec![AudioService::A2dp, AudioService::Handsfree, AudioService::Headset];
        *backend.fail.borrow_mut() = Some((AudioService::Handsfree, false));
        let mut state = ServiceState::default();
        assert!(state.connect(&backend, "a").is_err());
        assert_eq!(*backend.calls.borrow(), vec![
            (AudioService::A2dp, false), (AudioService::Handsfree, false), (AudioService::A2dp, true),
        ]);
        for service in [AudioService::A2dp, AudioService::Handsfree, AudioService::Headset] {
            assert!(backend.enabled.borrow().contains(&service));
        }
        assert!(state.restore.is_empty());
    }

    #[test]
    fn failed_reenable_keeps_original_snapshot_for_retry() {
        let backend = Backend::default();
        *backend.enabled.borrow_mut() = vec![AudioService::A2dp, AudioService::Handsfree];
        *backend.fail.borrow_mut() = Some((AudioService::A2dp, true));
        let mut state = ServiceState::default();
        assert!(state.connect(&backend, "a").is_err());
        assert_eq!(*backend.enabled.borrow(), vec![AudioService::Handsfree]);
        assert_eq!(state.restore["a"], vec![AudioService::A2dp, AudioService::Handsfree]);
        *backend.fail.borrow_mut() = None;
        backend.calls.borrow_mut().clear();
        state.connect(&backend, "a").unwrap();
        assert_eq!(*backend.calls.borrow(), vec![(AudioService::A2dp, true)]);
        assert!(state.restore.is_empty());
    }

    #[test]
    fn service_error_requires_independent_confirmation_of_requested_state() {
        for fail_code in [87, 0x8007_0057] {
            for enable in [false, true] {
                for fail_after_apply in [false, true] {
                    let backend = Backend { fail_code, fail_after_apply, ..Backend::default() };
                    if !enable {
                        backend.enabled.borrow_mut().push(AudioService::A2dp);
                    }
                    *backend.fail.borrow_mut() = Some((AudioService::A2dp, enable));
                    let result = set_service(&backend, "a", AudioService::A2dp, enable);
                    assert_eq!(result.is_ok(), fail_after_apply);
                    assert_eq!(backend.enabled.borrow().contains(&AudioService::A2dp),
                        if fail_after_apply { enable } else { !enable });
                }
            }
        }
    }

    #[test]
    fn already_requested_service_state_does_not_call_backend() {
        let backend = Backend::default();
        backend.enabled.borrow_mut().push(AudioService::A2dp);
        set_service(&backend, "a", AudioService::A2dp, true).unwrap();
        set_service(&backend, "a", AudioService::Handsfree, false).unwrap();
        assert!(backend.calls.borrow().is_empty());
    }

    #[test]
    fn repeated_disconnect_preserves_original_services_for_leave() {
        let backend = Backend::default();
        *backend.enabled.borrow_mut() = vec![AudioService::A2dp, AudioService::Handsfree];
        let mut state = ServiceState::default();
        state.disconnect(&backend, "a").unwrap();
        state.disconnect(&backend, "a").unwrap();
        assert!(backend.enabled.borrow().is_empty());
        state.restore(&backend, "a").unwrap();
        assert_eq!(*backend.enabled.borrow(), vec![AudioService::A2dp, AudioService::Handsfree]);
        assert!(state.restore.is_empty());
    }

    #[test]
    fn partial_disable_and_restore_errors_keep_retryable_snapshot() {
        let backend = Backend::default();
        *backend.enabled.borrow_mut() = vec![AudioService::A2dp, AudioService::Headset];
        *backend.fail.borrow_mut() = Some((AudioService::Headset, false));
        let mut state = ServiceState::default();
        assert!(state.disconnect(&backend, "a").is_err());
        assert_eq!(*backend.enabled.borrow(), vec![AudioService::Headset]);
        *backend.fail.borrow_mut() = Some((AudioService::A2dp, true));
        assert!(state.restore(&backend, "a").is_err());
        assert!(state.restore.contains_key("a"));
        *backend.fail.borrow_mut() = None;
        state.restore(&backend, "a").unwrap();
        assert_eq!(backend.enabled.borrow().len(), 2);
        assert!(backend.enabled.borrow().contains(&AudioService::A2dp));
        assert!(backend.enabled.borrow().contains(&AudioService::Headset));
    }

    #[test]
    fn empty_snapshot_and_unrelated_address_do_not_enable_services() {
        let backend = Backend::default();
        let mut state = ServiceState::default();
        state.disconnect(&backend, "a").unwrap();
        state.restore(&backend, "other").unwrap();
        state.restore(&backend, "a").unwrap();
        assert!(backend.enabled.borrow().is_empty());
    }
}
