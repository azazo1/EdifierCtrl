use std::time::Instant;

use edifier_protocol::{
    encode_tx, parse_notification, split_ble_chunks, Command, DeviceProfile, FrameDecoder,
    BLE_CHUNK_DELAY_MS, COMMAND_GAP_MS,
};
use edifier_session::readout_plan;
use tokio::sync::{broadcast, Mutex};
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
        self.transport.open(address, kind).await?;
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
        self.transport.close().await?;
        let mut st = self.state.lock().await;
        st.kind = None;
        st.address = None;
        st.last_tx = None;
        let _ = self.events.send(RuntimeEvent::BtState {
            connected: false,
            kind: None,
            address: None,
        });
        info!(target: "edifier_runtime", "已断开耳机控制通道");
        Ok(())
    }

    pub async fn send(&self, cmd: &Command) -> Result<(), TransportError> {
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
        loop {
            match self.transport.recv().await {
                Ok(bytes) if bytes.is_empty() => continue,
                Ok(bytes) => self.push_bytes(&bytes).await,
                Err(TransportError::Closed) => {
                    info!(target: "edifier_runtime", "控制通道关闭, 停止接收");
                    return Ok(());
                }
                Err(err) => return Err(err),
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
