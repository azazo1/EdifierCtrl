use std::collections::HashMap;

use edifier_runtime::TransportError;

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

#[derive(Default)]
pub(crate) struct ServiceState {
    restore: HashMap<String, Vec<AudioService>>,
}

impl ServiceState {
    pub(crate) fn disconnect(
        &mut self,
        backend: &impl ServiceControl,
        address: &str,
    ) -> Result<(), TransportError> {
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
        connect: bool,
    ) -> Result<(), TransportError> {
        let services = match self.restore.get(address) {
            Some(services) => services.clone(),
            None if connect => vec![AudioService::A2dp],
            None => return Ok(()),
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
        if let Err(error) = backend.set(address, *service, enable) {
            failure = Some(error);
        }
    }
    match failure {
        Some(error) => Err(error),
        None => Ok(()),
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
    }

    impl ServiceControl for Backend {
        fn enabled(&self, _address: &str) -> Result<Vec<AudioService>, TransportError> {
            Ok(self.enabled.borrow().clone())
        }

        fn set(&self, _address: &str, service: AudioService, enable: bool) -> Result<(), TransportError> {
            if *self.fail.borrow() == Some((service, enable)) {
                return Err(TransportError::Unavailable("模拟服务失败".into()));
            }
            let mut enabled = self.enabled.borrow_mut();
            enabled.retain(|value| *value != service);
            if enable { enabled.push(service); }
            Ok(())
        }
    }

    #[test]
    fn initial_connect_does_not_enable_handsfree() {
        let backend = Backend::default();
        ServiceState::default().restore(&backend, "a", true).unwrap();
        assert_eq!(*backend.enabled.borrow(), vec![AudioService::A2dp]);
    }

    #[test]
    fn repeated_disconnect_preserves_original_services_for_leave() {
        let backend = Backend::default();
        *backend.enabled.borrow_mut() = vec![AudioService::A2dp, AudioService::Handsfree];
        let mut state = ServiceState::default();
        state.disconnect(&backend, "a").unwrap();
        state.disconnect(&backend, "a").unwrap();
        assert!(backend.enabled.borrow().is_empty());
        state.restore(&backend, "a", false).unwrap();
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
        assert!(state.restore(&backend, "a", false).is_err());
        assert!(state.restore.contains_key("a"));
        *backend.fail.borrow_mut() = None;
        state.restore(&backend, "a", false).unwrap();
        assert_eq!(*backend.enabled.borrow(), vec![AudioService::A2dp, AudioService::Headset]);
    }

    #[test]
    fn empty_snapshot_and_unrelated_address_do_not_enable_services() {
        let backend = Backend::default();
        let mut state = ServiceState::default();
        state.disconnect(&backend, "a").unwrap();
        state.restore(&backend, "other", false).unwrap();
        state.restore(&backend, "a", false).unwrap();
        assert!(backend.enabled.borrow().is_empty());
    }
}
