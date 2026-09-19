use std::collections::{HashSet, VecDeque};

use edifier_protocol::AUDIO_CONNECT_GAP_MS;
use tokio::time::{sleep, sleep_until, timeout, timeout_at};

use super::*;

const AUDIO_TIMEOUT: Duration = Duration::from_secs(3);

pub(super) struct OperationState {
    pub(super) suppressed: HashSet<MacAddr>,
    pub(super) control_address: Option<MacAddr>,
    pub(super) can_control: bool,
}

impl OperationState {
    pub(super) fn new(can_control: bool) -> Self {
        Self { suppressed: HashSet::new(), control_address: None, can_control }
    }
}

pub(super) fn phase_mac(phase: Phase) -> Option<MacAddr> {
    match phase {
        Phase::Idle => None,
        Phase::WaitingRelease { headphone, .. }
        | Phase::WaitingFallbackGap { headphone, .. }
        | Phase::ConnectingAudio { headphone, .. }
        | Phase::Releasing { headphone, .. } => Some(headphone),
    }
}

impl<N: GroupNet, A: AudioControl> GroupHub<N, A> {
    pub(super) async fn apply(
        &self,
        operation: &mut OperationState,
        actions: Vec<Action>,
    ) -> Result<(), TransportError> {
        let mut queue: VecDeque<Action> = actions.into();
        let mut failure = None;
        while let Some(action) = queue.pop_front() {
            let result = match action {
                Action::Send(msg) => self.broadcast(msg).await.map(|()| Vec::new()),
                Action::ConnectAudio(mac) => {
                    match self.connect_and_select(operation, mac).await {
                        Ok(()) => {
                            self.update_holding(Some(mac.to_colon_string())).await;
                            Ok(self.machine.lock().await.on_audio_connected())
                        }
                        Err(err) => Err(err),
                    }
                }
                Action::DisconnectAudio(mac) => {
                    match self.disconnect_verified(operation, mac).await {
                        Ok(()) => {
                            if self.peer.lock().await.holding.as_deref() == Some(&mac.to_colon_string()) {
                                self.update_holding(None).await;
                            }
                            Ok(self.machine.lock().await.on_audio_disconnected())
                        }
                        Err(err) => Err(err),
                    }
                }
                Action::SuppressAutoreconnect { mac, suppress } => {
                    self.suppress(operation, mac, suppress).await.map(|()| Vec::new())
                }
                Action::SendHeadsetDisconnect => {
                    let target = phase_mac(self.machine.lock().await.phase());
                    match target {
                        Some(mac) => self.send_cd(operation, mac).await.map(|()| Vec::new()),
                        None => Err(TransportError::Connect("交接目标已失效".into())),
                    }
                }
                Action::Report(progress) => {
                    // Busy 属于被拒绝的新请求, 不能回滚已经执行中的交接.
                    match &progress {
                        HandoffProgress::Busy => {
                            failure.get_or_insert_with(|| TransportError::Unavailable("正在交接音频".into()));
                        }
                        HandoffProgress::Failed(reason) => {
                            failure.get_or_insert_with(|| TransportError::Connect(reason.clone()));
                        }
                        HandoffProgress::Done => info!(target: "edifier_runtime", "交接完成"),
                        _ => {}
                    }
                    let _ = self.events.send(RuntimeEvent::Handoff(progress));
                    Ok(Vec::new())
                }
            };
            match result {
                Ok(more) => {
                    for action in more.into_iter().rev() {
                        queue.push_front(action);
                    }
                }
                Err(err) => {
                    warn!(target: "edifier_runtime", %err, "交接动作失败");
                    if failure.is_none() {
                        let reason = err.to_string();
                        failure = Some(err);
                        // 失败后丢弃原动作队列, 只执行取消和恢复操作.
                        queue.clear();
                        queue.extend(self.machine.lock().await.on_audio_failed(&reason));
                        if queue.is_empty() {
                            queue.push_back(Action::Report(HandoffProgress::Failed(reason)));
                        }
                    }
                }
            }
        }
        failure.map_or(Ok(()), Err)
    }

    pub(super) async fn suppress(
        &self,
        operation: &mut OperationState,
        mac: MacAddr,
        suppress: bool,
    ) -> Result<(), TransportError> {
        if suppress {
            // 平台调用可能先产生副作用再被取消或返回错误, 必须提前登记.
            operation.suppressed.insert(mac);
        } else if !operation.suppressed.contains(&mac) {
            return Ok(());
        }
        let result = timeout(AUDIO_TIMEOUT, self.audio.suppress_autoreconnect(&mac.to_colon_string(), suppress))
            .await.map_err(|_| TransportError::Unavailable("设置重连抑制超时".into()))?;
        match result {
            Err(TransportError::Unsupported(reason)) if suppress => {
                warn!(target: "edifier_runtime", %reason, "平台不支持抑制重连, 继续验证实际释放状态");
                operation.suppressed.remove(&mac);
            }
            other => other?,
        }
        if !suppress {
            operation.suppressed.remove(&mac);
        }
        Ok(())
    }

    async fn connect_and_select(
        &self,
        operation: &mut OperationState,
        mac: MacAddr,
    ) -> Result<(), TransportError> {
        let addr = mac.to_colon_string();
        let new_connection = self.connect_verified(operation, mac).await?;
        let selected = timeout(AUDIO_TIMEOUT, self.audio.select_output(&addr))
            .await.map_err(|_| TransportError::Connect("选择音频输出超时".into()))
            .and_then(|result| result);
        if let Err(err) = selected {
            // 输出选择失败不等于音频未连接, 先保留已确认的状态供取消恢复.
            self.update_holding(Some(addr.clone())).await;
            if new_connection {
                let rollback = timeout(AUDIO_TIMEOUT, async {
                    self.audio.disconnect_audio(&addr).await?;
                    self.wait_audio(&addr, AudioState::Disconnected).await
                }).await;
                if !matches!(rollback, Ok(Ok(()))) {
                    warn!(target: "edifier_runtime", address = %addr, "输出选择失败后未能确认音频回滚");
                }
            }
            match timeout(AUDIO_TIMEOUT, self.audio.audio_state(&addr)).await {
                Ok(Ok(AudioState::Disconnected)) => self.update_holding(None).await,
                Ok(Ok(AudioState::Connected)) => self.update_holding(Some(addr)).await,
                _ => {}
            }
            self.announce_inner().await;
            return Err(err);
        }
        Ok(())
    }

    pub(super) async fn connect_verified(
        &self,
        operation: &mut OperationState,
        mac: MacAddr,
    ) -> Result<bool, TransportError> {
        self.suppress(operation, mac, false).await?;
        let addr = mac.to_colon_string();
        timeout(AUDIO_TIMEOUT, async {
            let before = self.audio.audio_state(&addr).await;
            if !matches!(before, Ok(AudioState::Connected)) {
                self.audio.connect_audio(&addr).await?;
            }
            self.wait_audio(&addr, AudioState::Connected).await?;
            // 只有明确从断开状态建立的连接才属于本次操作, 未知状态也保守保留.
            Ok(matches!(before, Ok(AudioState::Disconnected)))
        }).await.map_err(|_| TransportError::Connect("确认音频连接超时".into()))?
    }

    async fn disconnect_verified(
        &self,
        operation: &OperationState,
        mac: MacAddr,
    ) -> Result<(), TransportError> {
        let addr = mac.to_colon_string();
        if self.peer.lock().await.holding.as_deref() != Some(&addr) {
            return Err(TransportError::Connect("释放目标与本机持有地址不一致".into()));
        }
        let deadline_ms = match self.machine.lock().await.phase() {
            Phase::Releasing { headphone, deadline_ms, .. } if headphone == mac => deadline_ms,
            _ => return Err(TransportError::Connect("音频释放阶段或目标已失效".into())),
        };
        // 抑制重连等前置步骤已经消耗请求期限, 各阶段不能重新获得完整超时.
        // 预留回复时间, 单调时钟保证本次释放不会因系统时钟调整延长.
        let remaining = deadline_ms.saturating_sub(now_ms()).saturating_sub(250);
        if remaining == 0 {
            return Err(TransportError::Connect("音频释放仍待确认 (阶段: 系统断开前, 请求期限已耗尽)".into()));
        }
        let deadline = Instant::now() + Duration::from_millis(remaining);
        let mut context = Vec::new();
        info!(target: "edifier_runtime", address = %addr, remaining_ms = remaining, "请求系统释放音频");
        match timeout_at(deadline.min(Instant::now() + AUDIO_TIMEOUT), self.audio.disconnect_audio(&addr)).await {
            Ok(Ok(())) => {}
            Ok(Err(err)) => {
                warn!(target: "edifier_runtime", %err, "系统断开音频失败, 继续核验实际释放");
                context.push(format!("系统断开: {err}"));
            }
            Err(_) => {
                warn!(target: "edifier_runtime", "系统断开音频超时, 继续核验实际释放");
                context.push("系统断开调用超时".into());
            }
        }
        // 音频端点与控制通道的断开通知可能不同步, 先让系统状态收敛.
        let remaining = deadline.saturating_duration_since(Instant::now());
        let settle_deadline = Instant::now() + (remaining / 3).min(Duration::from_millis(750));
        if self.wait_release_until(&addr, settle_deadline).await.is_ok() {
            info!(target: "edifier_runtime", address = %addr, "已确认系统释放音频, 无需 CD 回退");
            return Ok(());
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        let mut stage = "系统断开后的状态确认";
        if operation.can_control && operation.control_address == Some(mac) && self.cd.is_some()
            && remaining > Duration::from_millis(100) {
            stage = "CD 回退后的状态确认";
            // CD 写入不能吃掉最终核验的时间. 控制通道已随系统断开时, 继续等音频状态.
            let cd_deadline = deadline - (remaining / 2).min(Duration::from_millis(500));
            let result = timeout_at(cd_deadline, self.send_cd_command(operation, mac)).await;
            let error = match result {
                Ok(Ok(())) => None,
                Ok(Err(err)) => Some(err.to_string()),
                Err(_) => Some("发送期限已耗尽".into()),
            };
            if let Some(err) = error {
                warn!(target: "edifier_runtime", address = %addr, %err, "CD 回退未完成, 继续核验系统释放");
                context.push(format!("CD 回退: {err}"));
            }
        }
        match self.wait_release_until(&addr, deadline).await {
            Ok(()) => {
                info!(target: "edifier_runtime", address = %addr, stage, "已确认实际音频释放");
                Ok(())
            }
            Err(last_state) => {
                context.insert(0, last_state);
                Err(TransportError::Connect(format!("音频释放仍待确认 (阶段: {stage}, {})", context.join("; "))))
            }
        }
    }

    async fn wait_release_until(&self, addr: &str, deadline: Instant) -> Result<(), String> {
        let mut last_state = "未取得音频状态".to_owned();
        while Instant::now() < deadline {
            last_state = match timeout_at(deadline, self.audio.audio_state(addr)).await {
                Ok(Ok(AudioState::Disconnected)) => return Ok(()),
                Ok(Ok(AudioState::Connected)) => "音频仍连接".into(),
                Ok(Ok(AudioState::Connecting)) => "音频仍在连接".into(),
                Ok(Ok(AudioState::Unknown)) => "音频状态未知".into(),
                Ok(Err(err)) => format!("读取音频状态失败: {err}"),
                Err(_) => return Err(format!("读取音频状态超时; {last_state}")),
            };
            sleep_until(deadline.min(Instant::now() + Duration::from_millis(50))).await;
        }
        Err(last_state)
    }

    async fn wait_audio(&self, addr: &str, expected: AudioState) -> Result<(), TransportError> {
        loop {
            let actual = self.audio.audio_state(addr).await?;
            if actual == expected {
                return Ok(());
            }
            sleep(Duration::from_millis(50)).await;
        }
    }

    async fn send_cd(&self, operation: &OperationState, mac: MacAddr) -> Result<(), TransportError> {
        self.send_cd_command(operation, mac).await?;
        // 请求接管侧从 CD 写入完成后计时, 再发起本机连接. 释放侧只核验真实断开.
        sleep(Duration::from_millis(AUDIO_CONNECT_GAP_MS)).await;
        Ok(())
    }

    async fn send_cd_command(&self, operation: &OperationState, mac: MacAddr) -> Result<(), TransportError> {
        if !operation.can_control || operation.control_address != Some(mac) {
            return Err(TransportError::Connect("控制通道不属于交接目标".into()));
        }
        let cd = self.cd.as_ref()
            .ok_or_else(|| TransportError::Unavailable("没有耳机控制通道".into()))?;
        info!(target: "edifier_runtime", address = %mac.to_colon_string(), "发送 CD 释放音频");
        timeout(AUDIO_TIMEOUT, cd.send_headset_disconnect())
            .await.map_err(|_| TransportError::Write("发送 CD 超时".into()))??;
        Ok(())
    }
}
