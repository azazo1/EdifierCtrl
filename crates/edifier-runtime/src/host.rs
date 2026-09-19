use std::time::Instant;

use edifier_protocol::{
    encode_tx, parse_notification, split_ble_chunks, Command, DeviceProfile, FrameDecoder,
    BLE_CHUNK_DELAY_MS, COMMAND_GAP_MS,
};
use edifier_session::readout_plan;
use tokio::sync::{broadcast, watch, Mutex};
use tokio::time::{sleep, Duration};
use tracing::{debug, info, warn};

use crate::events::RuntimeEvent;
use crate::transport::{HeadsetTransport, LinkKind, ScanResult, TransportError};

struct LinkState {
    kind: Option<LinkKind>,
    address: Option<String>,
    last_tx: Option<Instant>,
}

/// 把传输层和协议编解码拼在一起: 节流, BLE 分片, 通知分发.
pub struct HeadsetHost<T: HeadsetTransport> {
    transport: T,
    decoder: Mutex<FrameDecoder>,
    state: Mutex<LinkState>,
    operations: Mutex<()>,
    generation: watch::Sender<u64>,
    events: broadcast::Sender<RuntimeEvent>,
}

impl<T: HeadsetTransport> HeadsetHost<T> {
    pub fn new(transport: T) -> Self {
        let (events, _) = broadcast::channel(64);
        Self {
            transport,
            decoder: Mutex::new(FrameDecoder::new()),
            state: Mutex::new(LinkState {
                kind: None,
                address: None,
                last_tx: None,
            }),
            operations: Mutex::new(()),
            generation: watch::channel(0).0,
            events,
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<RuntimeEvent> {
        self.events.subscribe()
    }

    pub fn transport(&self) -> &T {
        &self.transport
    }

    pub async fn scan(&self, kind: LinkKind) -> Result<Vec<ScanResult>, TransportError> {
        info!(target: "edifier_runtime", ?kind, "开始扫描耳机");
        self.transport.scan(kind).await
    }

    pub async fn connect(&self, address: &str, kind: LinkKind) -> Result<(), TransportError> {
        let _operation = self.operations.lock().await;
        if self.state.lock().await.kind.is_some() {
            self.clear_link().await;
            self.transport.close().await?;
        }
        *self.decoder.lock().await = FrameDecoder::new();
        if let Err(err) = self.transport.open(address, kind).await {
            let _ = self.transport.close().await;
            self.clear_link().await;
            return Err(err);
        }
        {
            let mut st = self.state.lock().await;
            st.kind = Some(kind);
            st.address = Some(address.to_string());
            st.last_tx = None;
        }
        let _ = self.events.send(RuntimeEvent::BtState {
            connected: true,
            kind: Some(kind),
            address: Some(address.to_string()),
        });
        info!(target: "edifier_runtime", address, ?kind, "已连接耳机控制通道");
        Ok(())
    }

    pub async fn disconnect(&self) -> Result<(), TransportError> {
        let _operation = self.operations.lock().await;
        self.clear_link().await;
        let result = self.transport.close().await;
        info!(target: "edifier_runtime", "已断开耳机控制通道");
        result
    }

    pub async fn connected(&self) -> bool {
        self.state.lock().await.kind.is_some()
    }

    pub async fn connected_address(&self) -> Option<String> {
        self.state.lock().await.address.clone()
    }

    async fn clear_link(&self) {
        self.generation.send_modify(|generation| *generation = generation.wrapping_add(1));
        let mut st = self.state.lock().await;
        let was_connected = st.kind.take().is_some();
        st.address = None;
        st.last_tx = None;
        *self.decoder.lock().await = FrameDecoder::new();
        if was_connected {
            let _ = self.events.send(RuntimeEvent::BtState {
                connected: false,
                kind: None,
                address: None,
            });
        }
    }

    pub async fn send(&self, cmd: &Command) -> Result<(), TransportError> {
        let _operation = self.operations.lock().await;
        let body = cmd
            .to_body()
            .map_err(|e| TransportError::Write(e.to_string()))?;
        let frame = encode_tx(&body).map_err(|e| TransportError::Write(e.to_string()))?;
        let kind = {
            let st = self.state.lock().await;
            st.kind.ok_or_else(|| TransportError::Connect("尚未连接".into()))?
        };
        self.pace().await;
        match kind {
            LinkKind::Rfcomm => {
                self.transport.write(&frame).await?;
            }
            LinkKind::Ble => {
                for chunk in split_ble_chunks(&frame) {
                    self.transport.write(&chunk).await?;
                    sleep(Duration::from_millis(BLE_CHUNK_DELAY_MS)).await;
                }
            }
        }
        self.state.lock().await.last_tx = Some(Instant::now());
        debug!(
            target: "edifier_runtime",
            label = cmd.label(),
            len = frame.len(),
            "已发送命令"
        );
        Ok(())
    }

    pub async fn readout(&self, profile: &DeviceProfile) -> Result<(), TransportError> {
        info!(
            target: "edifier_runtime",
            profile = profile.id.as_str(),
            "开始读取耳机状态"
        );
        for cmd in readout_plan(profile) {
            self.send(&cmd).await?;
        }
        Ok(())
    }

    /// 持续把 recv 字节推进解码器, 直到通道关闭.
    pub async fn pump(&self) -> Result<(), TransportError> {
        let mut generation = self.generation.subscribe();
        if !self.connected().await {
            return Ok(());
        }
        loop {
            let received = tokio::select! {
                biased;
                _ = generation.changed() => return Ok(()),
                result = self.transport.recv() => result,
            };
            let _operation = self.operations.lock().await;
            if generation.has_changed().unwrap_or(true) {
                return Ok(());
            }
            match received {
                Ok(bytes) if bytes.is_empty() => {
                    drop(_operation);
                    tokio::task::yield_now().await;
                }
                Ok(bytes) => self.push_bytes(&bytes).await,
                Err(err) => {
                    self.clear_link().await;
                    let _ = self.transport.close().await;
                    return if matches!(err, TransportError::Closed) {
                        debug!(target: "edifier_runtime", "控制通道关闭, 停止接收");
                        Ok(())
                    } else {
                        Err(err)
                    };
                }
            }
        }
    }

    async fn push_bytes(&self, bytes: &[u8]) {
        let mut decoder = self.decoder.lock().await;
        for item in decoder.push(bytes) {
            match item {
                Ok(frame) => {
                    let note = parse_notification(&frame);
                    debug!(target: "edifier_runtime", ?note, "收到耳机通知");
                    let _ = self.events.send(RuntimeEvent::Headset(note));
                }
                Err(err) => warn!(target: "edifier_runtime", %err, "解码失败"),
            }
        }
    }

    async fn pace(&self) {
        let wait = {
            let st = self.state.lock().await;
            st.last_tx.and_then(|t| {
                let elapsed = t.elapsed();
                let gap = Duration::from_millis(COMMAND_GAP_MS);
                gap.checked_sub(elapsed)
            })
        };
        if let Some(d) = wait {
            sleep(d).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use edifier_protocol::{to_hex, Command, Notification, ProfileId};

    use super::*;
    use crate::mock::MockTransport;
    use crate::transport::ScanResult;

    fn sample_scan() -> ScanResult {
        ScanResult {
            address: "11:22:33:44:55:66".into(),
            name: "W820NB".into(),
            kind: LinkKind::Rfcomm,
            service_uuid: None,
        }
    }

    #[tokio::test]
    async fn send_query_battery_writes_frame() {
        let mock = MockTransport::new(vec![sample_scan()]);
        let host = HeadsetHost::new(mock);
        host.connect("11:22:33:44:55:66", LinkKind::Rfcomm)
            .await
            .unwrap();
        host.send(&Command::QueryBattery).await.unwrap();
        let written = host.transport().written().await;
        assert_eq!(written.len(), 1);
        assert_eq!(to_hex(&written[0]), "AA01D02194");
    }

    #[tokio::test]
    async fn pump_emits_battery_notification() {
        let mock = MockTransport::new(vec![sample_scan()]);
        mock.inject(edifier_protocol::parse_hex("BB02D04D21F3").unwrap());
        let host = HeadsetHost::new(mock);
        host.connect("11:22:33:44:55:66", LinkKind::Rfcomm)
            .await
            .unwrap();
        let mut rx = host.subscribe();
        host.transport().close_incoming();
        host.pump().await.unwrap();
        let ev = rx.try_recv().expect("应收到通知");
        match ev {
            RuntimeEvent::Headset(Notification::Battery(p)) => assert_eq!(p, 77),
            other => panic!("意外事件 {other:?}"),
        }
        assert!(matches!(rx.try_recv().unwrap(), RuntimeEvent::BtState { connected: false, .. }));
        assert!(!host.connected().await);
        assert!(host.send(&Command::QueryBattery).await.is_err());
    }

    #[tokio::test]
    async fn reconnect_discards_partial_frame() {
        let host = HeadsetHost::new(MockTransport::new(Vec::new()));
        host.connect("11:22:33:44:55:66", LinkKind::Rfcomm).await.unwrap();
        host.push_bytes(&[0xBB, 0x02, 0xD0]).await;
        host.connect("AA:BB:CC:DD:EE:FF", LinkKind::Rfcomm).await.unwrap();
        let mut events = host.subscribe();
        host.push_bytes(&[0x4D, 0x21, 0xF3]).await;
        assert!(events.try_recv().is_err());
        host.push_bytes(&[0xBB, 0x02, 0xD0, 0x4D, 0x21, 0xF3]).await;
        assert!(matches!(events.try_recv().unwrap(), RuntimeEvent::Headset(Notification::Battery(77))));
    }

    #[tokio::test]
    async fn disconnect_cancels_pending_receive() {
        let host = std::sync::Arc::new(HeadsetHost::new(MockTransport::new(Vec::new())));
        host.connect("11:22:33:44:55:66", LinkKind::Rfcomm).await.unwrap();
        let pump = host.clone();
        let task = tokio::spawn(async move { pump.pump().await });
        tokio::task::yield_now().await;
        tokio::time::timeout(Duration::from_secs(1), host.disconnect()).await.unwrap().unwrap();
        tokio::time::timeout(Duration::from_secs(1), task).await.unwrap().unwrap().unwrap();
        assert!(!host.connected().await);
    }

    #[tokio::test]
    async fn concurrent_ble_commands_do_not_interleave_chunks() {
        let host = HeadsetHost::new(MockTransport::new(Vec::new()));
        host.connect("11:22:33:44:55:66", LinkKind::Ble).await.unwrap();
        let first = Command::SetName("abcdefghijabcdefghij".into());
        let second = Command::SetName("01234567890123456789".into());
        let (a, b) = tokio::join!(host.send(&first), host.send(&second));
        a.unwrap();
        b.unwrap();
        let expected: Vec<Vec<u8>> = [&first, &second].into_iter().flat_map(|command| {
            split_ble_chunks(&encode_tx(&command.to_body().unwrap()).unwrap())
        }).collect();
        assert_eq!(host.transport().written().await, expected);
    }

    #[tokio::test]
    async fn ble_write_is_chunked() {
        let mock = MockTransport::new(vec![ScanResult {
            address: "11:22:33:44:55:66".into(),
            name: "W820NB".into(),
            kind: LinkKind::Ble,
            service_uuid: None,
        }]);
        let host = HeadsetHost::new(mock);
        host.connect("11:22:33:44:55:66", LinkKind::Ble)
            .await
            .unwrap();
        host.send(&Command::SetName("abcdefghijabcdefghij".into()))
            .await
            .unwrap();
        let written = host.transport().written().await;
        assert!(written.len() >= 2, "长名字应分成多片 {:?}", written.len());
        assert!(written.iter().all(|c| c.len() <= 20));
    }

    #[tokio::test]
    async fn readout_sends_battery_first() {
        let mock = MockTransport::new(vec![sample_scan()]);
        let host = HeadsetHost::new(mock);
        host.connect("11:22:33:44:55:66", LinkKind::Rfcomm)
            .await
            .unwrap();
        host.readout(DeviceProfile::by_id(ProfileId::W200BtPlus))
            .await
            .unwrap();
        let written = host.transport().written().await;
        assert!(!written.is_empty());
        assert_eq!(to_hex(&written[0]), "AA01D02194");
    }
}
