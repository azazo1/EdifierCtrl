//! 平台蓝牙与音频交接的异步边界.

pub mod cd;
pub mod events;
pub mod group;
pub mod host;
pub mod mock;
pub mod net;
pub mod transport;
pub mod udp;

pub use cd::{CdFallback, MockCd};
pub use events::RuntimeEvent;
pub use group::{now_ms, random_nonce, GroupHub};
pub use host::HeadsetHost;
pub use mock::{MockAudio, MockTransport};
pub use net::{Datagram, GroupNet, LoopbackNet, GROUP_PORT};
pub use transport::{
    AudioControl, AudioState, HeadsetTransport, LinkKind, ScanResult, TransportError,
};
pub use udp::{UdpGroupNet, GROUP_MCAST};
