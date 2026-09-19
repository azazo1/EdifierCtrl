use edifier_protocol::{
    parse_hex, parse_notification, to_hex, verify_checksum, FrameDecoder, IncomingFrame,
    Notification, TX_HEAD,
};
use serde::Serialize;

#[derive(Serialize)]
pub struct ParsedFrame {
    pub dir: &'static str,
    pub head: u8,
    pub body: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notification: Option<NotifyJson>,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NotifyJson {
    SoundEffect { effect: String },
    GameMode { on: bool },
    Battery { percent: u8 },
    Ldac { mode: String },
    PromptVolume { volume: u8 },
    ShutdownTimerEnabled { on: bool },
    ShutdownTimer { minutes: u8 },
    AutoPowerOff { on: bool },
    Mac { address: String },
    Firmware { version: String },
    Noise { mode: String, ambient_volume: i8 },
    Name { name: String },
    ControlSettings {
        normal: bool,
        reduction: bool,
        ambient: bool,
    },
    Ack { head: u8, body: String },
    Unknown { head: u8, body: String },
}

impl NotifyJson {
    pub fn from_notification(n: &Notification) -> Self {
        match n {
            Notification::SoundEffect(fx) => Self::SoundEffect {
                effect: format!("{fx:?}").to_ascii_lowercase(),
            },
            Notification::GameMode(on) => Self::GameMode { on: *on },
            Notification::Battery(p) => Self::Battery { percent: *p },
            Notification::Ldac(mode) => Self::Ldac {
                mode: format!("{mode:?}").to_ascii_lowercase(),
            },
            Notification::PromptVolume(v) => Self::PromptVolume { volume: *v },
            Notification::ShutdownTimerEnabled(on) => Self::ShutdownTimerEnabled { on: *on },
            Notification::ShutdownTimer { minutes } => Self::ShutdownTimer { minutes: *minutes },
            Notification::AutoPowerOff(on) => Self::AutoPowerOff { on: *on },
            Notification::Mac(mac) => Self::Mac {
                address: format!(
                    "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
                    mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
                ),
            },
            Notification::Firmware(fw) => Self::Firmware {
                version: format!("{:02X}.{:02X}.{:02X}", fw[0], fw[1], fw[2]),
            },
            Notification::Noise {
                mode,
                ambient_volume,
            } => Self::Noise {
                mode: format!("{mode:?}").to_ascii_lowercase(),
                ambient_volume: *ambient_volume,
            },
            Notification::Name(name) => Self::Name { name: name.clone() },
            Notification::ControlSettings(cs) => Self::ControlSettings {
                normal: cs.normal,
                reduction: cs.reduction,
                ambient: cs.ambient,
            },
            Notification::Ack { head, body } => Self::Ack {
                head: *head,
                body: to_hex(body),
            },
            Notification::Unknown { head, body } => Self::Unknown {
                head: *head,
                body: to_hex(body),
            },
        }
    }
}

pub fn parse_frame_hex(frame_hex: &str) -> Result<ParsedFrame, String> {
    let raw = parse_hex(frame_hex).map_err(|e| e.to_string())?;
    if raw.first() == Some(&TX_HEAD) {
        verify_checksum(&raw).map_err(|e| e.to_string())?;
        let end = raw.len().saturating_sub(2);
        let body = if end >= 2 { to_hex(&raw[2..end]) } else { String::new() };
        return Ok(ParsedFrame {
            dir: "tx",
            head: TX_HEAD,
            body,
            notification: None,
        });
    }
    let mut dec = FrameDecoder::new();
    let mut frames = dec.push(&raw);
    if frames.is_empty() {
        return Err("帧不完整".into());
    }
    let frame = frames.remove(0).map_err(|e| e.to_string())?;
    Ok(parsed_incoming(&frame))
}

pub fn parsed_incoming(frame: &IncomingFrame) -> ParsedFrame {
    let note = parse_notification(frame);
    ParsedFrame {
        dir: "rx",
        head: frame.head,
        body: to_hex(&frame.body),
        notification: Some(NotifyJson::from_notification(&note)),
    }
}

pub fn decoder_push_hex(decoder: &mut FrameDecoder, hex: &str) -> Result<Vec<ParsedFrame>, String> {
    let raw = parse_hex(hex).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for item in decoder.push(&raw) {
        match item {
            Ok(frame) => out.push(parsed_incoming(&frame)),
            Err(err) => return Err(err.to_string()),
        }
    }
    Ok(out)
}
