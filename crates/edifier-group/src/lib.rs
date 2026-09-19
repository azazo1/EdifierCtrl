//! 局域网组: 口令派生, 报文 HMAC, 音频交接状态机.

pub mod envelope;
pub mod handoff;
pub mod key;
pub mod mac;
pub mod message;

pub use envelope::{open, seal, EnvelopeError};
pub use handoff::{Action, HandoffMachine, HandoffProgress, Phase, HANDOFF_DEADLINE_MS};
pub use key::{derive_group, GroupId, GroupKey};
pub use mac::MacAddr;
pub use message::{Envelope, GroupMessage, PeerInfo, ENVELOPE_VERSION};
