use std::sync::{Arc, Mutex};
use std::time::Duration;

use edifier_bt_linux::{LinuxAudio, LinuxHeadset};
use edifier_protocol::{Command, DeviceProfile};
use edifier_runtime::{
    CdFallback, GroupHub, HeadsetHost, LinkKind, RuntimeEvent, ScanResult, UdpGroupNet,
};
use tokio::sync::broadcast;
use tracing::info;

pub type Hub = Arc<GroupHub<UdpGroupNet, Arc<LinuxAudio>>>;

pub struct AppState {
    pub rt: tokio::runtime::Runtime,
    pub host: Arc<HeadsetHost<LinuxHeadset>>,
    pub audio: Arc<LinuxAudio>,
    pub group: Mutex<Option<Hub>>,
    pub host_events: Mutex<broadcast::Receiver<RuntimeEvent>>,
    pub group_events: Mutex<Option<broadcast::Receiver<RuntimeEvent>>>,
    pub last_mac: Mutex<Option<String>>,
    pub last_kind: Mutex<LinkKind>,
}

impl AppState {
    pub fn new() -> Arc<Self> {
        let rt = tokio::runtime::Runtime::new().expect("tokio");
        let host = Arc::new(HeadsetHost::new(LinuxHeadset::new()));
        let audio = Arc::new(LinuxAudio::new());
        let host_events = Mutex::new(host.subscribe());
        let pump = host.clone();
        rt.spawn(async move {
            loop {
                let _ = pump.pump().await;
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        });
        Arc::new(Self {
            rt,
            host,
            audio,
            group: Mutex::new(None),
            host_events,
            group_events: Mutex::new(None),
            last_mac: Mutex::new(None),
            last_kind: Mutex::new(LinkKind::Rfcomm),
        })
    }

    pub fn scan(&self, kind: LinkKind) -> Result<Vec<ScanResult>, String> {
        *self.last_kind.lock().expect("kind") = kind;
        self.rt
            .block_on(self.host.scan(kind))
            .map_err(|e| e.to_string())
    }

    pub fn connect(&self, address: &str) -> Result<(), String> {
        let kind = *self.last_kind.lock().expect("kind");
        self.rt
            .block_on(self.host.connect(address, kind))
            .map_err(|e| e.to_string())?;
        *self.last_mac.lock().expect("mac") = Some(address.to_string());
        self.sync_holding();
        info!(target: "edifier_linux", address, "已连接控制通道");
        Ok(())
    }

    pub fn disconnect(&self) -> Result<(), String> {
        self.rt
            .block_on(self.host.disconnect())
            .map_err(|e| e.to_string())?;
        *self.last_mac.lock().expect("mac") = None;
        self.sync_holding();
        Ok(())
    }

    pub fn send(&self, cmd: &Command) -> Result<(), String> {
        self.rt
            .block_on(self.host.send(cmd))
            .map_err(|e| e.to_string())
    }

    pub fn readout(&self) -> Result<(), String> {
        let profile = DeviceProfile::by_key("basedevice")
            .ok_or_else(|| "未知机型档案".to_string())?;
        self.rt
            .block_on(self.host.readout(profile))
            .map_err(|e| e.to_string())
    }

    pub fn join_group(&self, pass: &str) -> Result<String, String> {
        if self.group.lock().expect("hub").is_some() {
            return Err("已经加入组".into());
        }
        let net = self
            .rt
            .block_on(UdpGroupNet::bind())
            .map_err(|e| e.to_string())?;
        let id = format!("gtk-{}", std::process::id());
        let hub = Arc::new(GroupHub::new(
            pass,
            id,
            net,
            self.audio.clone(),
            Some(self.host.clone() as Arc<dyn CdFallback>),
        ));
        let run = hub.clone();
        self.rt.spawn(async move {
            let _ = run.run().await;
        });
        let gid = hub.group_id_hex();
        *self.group_events.lock().expect("events") = Some(hub.subscribe());
        *self.group.lock().expect("hub") = Some(hub);
        self.sync_holding();
        Ok(gid)
    }

    pub fn poll_event(&self) -> Option<String> {
        if let Ok(ev) = self.host_events.lock().expect("host ev").try_recv() {
            return Some(fmt_event(&ev));
        }
        if let Some(rx) = self.group_events.lock().expect("group ev").as_mut() {
            if let Ok(ev) = rx.try_recv() {
                return Some(fmt_event(&ev));
            }
        }
        None
    }

    pub fn peers(&self) -> Result<Vec<edifier_group::PeerInfo>, String> {
        let hub = self
            .group
            .lock()
            .expect("hub")
            .clone()
            .ok_or_else(|| "尚未加入组".to_string())?;
        Ok(self.rt.block_on(hub.peers()))
    }

    pub fn claim(&self, mac: &str) -> Result<(), String> {
        let hub = self
            .group
            .lock()
            .expect("hub")
            .clone()
            .ok_or_else(|| "尚未加入组".to_string())?;
        self.rt.block_on(hub.claim(mac)).map_err(|e| e.to_string())
    }

    pub fn claim_peer(&self, peer_id: &str) -> Result<String, String> {
        let peers = self.peers()?;
        let peer = peers
            .iter()
            .find(|p| p.id == peer_id)
            .ok_or_else(|| "组里没有这个成员".to_string())?;
        let mac = peer
            .holding
            .as_deref()
            .ok_or_else(|| "该成员没有持有耳机".to_string())?;
        self.claim(mac)?;
        Ok(format!("已向 {} 请求接管 {mac}", peer.hostname))
    }

    fn sync_holding(&self) {
        let mac = self.last_mac.lock().expect("mac").clone();
        if let Some(hub) = self.group.lock().expect("hub").clone() {
            self.rt.block_on(hub.adopt_headset(mac));
        }
    }
}

fn fmt_event(ev: &RuntimeEvent) -> String {
    match ev {
        RuntimeEvent::BtState {
            connected,
            address,
            ..
        } => format!(
            "bt connected={connected} {}",
            address.as_deref().unwrap_or("-")
        ),
        RuntimeEvent::Headset(n) => format!("headset {n:?}"),
        RuntimeEvent::Audio(state) => format!("audio {state:?}"),
        RuntimeEvent::Peer(peer) => format!(
            "peer {} holding={}",
            peer.hostname,
            peer.holding.as_deref().unwrap_or("-")
        ),
        RuntimeEvent::Handoff(progress) => format!("handoff {progress:?}"),
        RuntimeEvent::Message(text) => text.clone(),
    }
}
