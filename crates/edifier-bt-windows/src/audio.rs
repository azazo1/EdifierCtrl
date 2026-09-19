use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use edifier_runtime::{AudioControl, AudioState, TransportError};
use tracing::info;

use crate::{a2dp, com};

mod observed;
pub(crate) mod services;
mod state;

/// 通过公开的服务开关请求连接, 通过系统音频端点确认实际状态.
pub struct WindowsAudio {
    services: Arc<Mutex<services::ServiceState>>,
}

impl WindowsAudio {
    pub fn new() -> Self {
        Self { services: Arc::new(Mutex::new(services::ServiceState::default())) }
    }

    async fn update_services(&self, address: &str, release: bool, connect: bool) -> Result<(), TransportError> {
        let address = com::format_addr(com::parse_addr(address)?);
        let services = self.services.clone();
        com::blocking(move || {
            // 锁在后台闭包内持有, 调用方超时取消后也不会与后续恢复交错.
            let mut services = services.lock()
                .map_err(|err| TransportError::Unavailable(format!("音频服务状态锁: {err}")))?;
            if release {
                services.disconnect(&a2dp::Win32Services, &address)?;
            } else if connect {
                let observed = state::audio_state(&address);
                info!(target: "edifier_bt_windows", address, ?observed, "请求连接前核验音频端点");
                // 读取失败不能当成未连接, 避免关闭实际上仍在使用的服务.
                if observed? == AudioState::Connected {
                    services.observed_connected(&address);
                } else {
                    services.connect(&a2dp::Win32Services, &address)?;
                }
            } else {
                services.restore(&a2dp::Win32Services, &address)?;
            }
            info!(target: "edifier_bt_windows", address, release, connect, "音频服务请求完成, 实际连接由系统确认");
            Ok(())
        }).await
    }
}

impl Default for WindowsAudio {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AudioControl for WindowsAudio {
    async fn audio_state(&self, address: &str) -> Result<AudioState, TransportError> {
        let address = com::format_addr(com::parse_addr(address)?);
        let services = self.services.clone();
        com::blocking(move || {
            let mut services = services.lock()
                .map_err(|err| TransportError::Unavailable(format!("音频服务状态锁: {err}")))?;
            let observed = state::audio_state(&address)?;
            if observed == AudioState::Connected {
                services.observed_connected(&address);
            }
            Ok(observed)
        }).await
    }

    async fn connect_audio(&self, address: &str) -> Result<(), TransportError> {
        self.update_services(address, false, true).await
    }

    async fn disconnect_audio(&self, address: &str) -> Result<(), TransportError> {
        self.update_services(address, true, false).await
    }

    async fn suppress_autoreconnect(
        &self,
        address: &str,
        suppress: bool,
    ) -> Result<(), TransportError> {
        // Windows 无独立的抑制接口, 退出时恢复原服务可能触发系统自动重连.
        self.update_services(address, suppress, false).await
    }
}
