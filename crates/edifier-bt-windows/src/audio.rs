use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use edifier_runtime::{AudioControl, AudioState, TransportError};
use tracing::info;

use crate::{a2dp, com};

mod observed;
pub(crate) mod services;
mod state;

/// 通过服务开关抑制重连, 释放时再请求断开 ACL; 用 ACL 与音频端点确认实际状态.
pub struct WindowsAudio {
    services: Arc<Mutex<services::ServiceState>>,
    operation_gate: Arc<tokio::sync::Mutex<()>>,
}

impl WindowsAudio {
    pub fn new() -> Self {
        Self {
            services: Arc::new(Mutex::new(services::ServiceState::default())),
            operation_gate: Arc::new(tokio::sync::Mutex::new(())),
        }
    }

    async fn update_services(
        &self,
        address: &str,
        release: bool,
        connect: bool,
        drop_acl: bool,
    ) -> Result<(), TransportError> {
        let address = com::format_addr(com::parse_addr(address)?);
        let services = self.services.clone();
        // 进入阻塞线程前按序排队. guard 随闭包存活, 取消等待也不会让恢复越过尚未结束的写操作.
        let operation_guard = self.operation_gate.clone().lock_owned().await;
        com::blocking(move || {
            let _operation_guard = operation_guard;
            let mut services = services.lock()
                .map_err(|err| TransportError::Unavailable(format!("音频服务状态锁: {err}")))?;
            if release {
                let disable = services.disconnect(&a2dp::Win32Services, &address);
                let acl = if drop_acl {
                    a2dp::disconnect_acl(&address)
                } else {
                    Ok(())
                };
                match (disable, acl) {
                    (Ok(()), Ok(())) => {}
                    (Err(err), Ok(())) | (Ok(()), Err(err)) => return Err(err),
                    (Err(a), Err(b)) => {
                        return Err(TransportError::Connect(format!(
                            "关闭音频服务失败: {a}; 断开 ACL 失败: {b}"
                        )));
                    }
                }
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
            info!(
                target: "edifier_bt_windows",
                address, release, connect, drop_acl,
                "音频服务请求完成, 实际连接由系统确认"
            );
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
    fn operation_timeout(&self) -> std::time::Duration {
        // A2DP 与免提服务顺序移除/恢复可能超过默认 3 秒, 不并发操作驱动.
        std::time::Duration::from_secs(15)
    }

    async fn audio_state(&self, address: &str) -> Result<AudioState, TransportError> {
        let address = com::format_addr(com::parse_addr(address)?);
        let services = self.services.clone();
        let Ok(operation_guard) = self.operation_gate.clone().try_lock_owned() else {
            // 服务修改或另一状态查询尚未完成, 不为每次轮询堆积阻塞线程.
            return Ok(AudioState::Unknown);
        };
        com::blocking(move || {
            let _operation_guard = operation_guard;
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
        self.update_services(address, false, true, false).await
    }

    async fn disconnect_audio(&self, address: &str) -> Result<(), TransportError> {
        self.update_services(address, true, false, true).await
    }

    async fn suppress_autoreconnect(
        &self,
        address: &str,
        suppress: bool,
    ) -> Result<(), TransportError> {
        // Windows 无独立的抑制接口, 退出时恢复原服务可能触发系统自动重连.
        self.update_services(address, suppress, false, false).await
    }
}
