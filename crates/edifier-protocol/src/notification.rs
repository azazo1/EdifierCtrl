use crate::command::{ControlSettings, LdacMode, NoiseMode, SoundEffect};
use crate::constants::AMBIENT_VOLUME_OFFSET;
use crate::packet::IncomingFrame;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Notification {
    SoundEffect(SoundEffect),
    GameMode(bool),
    Battery(u8),
    Ldac(LdacMode),
    PromptVolume(u8),
    ShutdownTimerEnabled(bool),
    ShutdownTimer { minutes: u8 },
    AutoPowerOff(bool),
    Mac([u8; 6]),
    Firmware([u8; 3]),
    Noise {
        mode: NoiseMode,
        ambient_volume: i8,
    },
    Name(String),
    ControlSettings(ControlSettings),
    Ack {
        head: u8,
        body: Vec<u8>,
    },
    Unknown {
        head: u8,
        body: Vec<u8>,
    },
}

fn noise_mode(v: u8) -> Option<NoiseMode> {
    match v {
        0x01 => Some(NoiseMode::Normal),
        0x02 => Some(NoiseMode::Reduction),
        0x03 => Some(NoiseMode::Ambient),
        _ => None,
    }
}

fn sound_effect(v: u8) -> Option<SoundEffect> {
    match v {
        0x00 => Some(SoundEffect::Normal),
        0x01 => Some(SoundEffect::Pop),
        0x02 => Some(SoundEffect::Classical),
        0x03 => Some(SoundEffect::Rock),
        _ => None,
    }
}

fn ldac_mode(v: u8) -> Option<LdacMode> {
    match v {
        0x00 => Some(LdacMode::Off),
        0x01 => Some(LdacMode::Rate48k),
        0x02 => Some(LdacMode::Rate96k),
        _ => None,
    }
}

pub fn parse_notification(frame: &IncomingFrame) -> Notification {
    let head = frame.head;
    let body = &frame.body;
    if body.is_empty() {
        return Notification::Unknown {
            head,
            body: body.clone(),
        };
    }

    if frame.is_ack() {
        return Notification::Ack {
            head,
            body: body.clone(),
        };
    }

    if !frame.is_response() {
        return Notification::Unknown {
            head,
            body: body.clone(),
        };
    }

    let cmd = body[0];
    match (cmd, body.len()) {
        (0xD5, 2) => sound_effect(body[1])
            .map(Notification::SoundEffect)
            .unwrap_or(unknown(frame)),
        (0x08, 2) => Notification::GameMode(body[1] == 0x01),
        (0xD0, 2) => Notification::Battery(body[1]),
        (0x48, 2) => ldac_mode(body[1])
            .map(Notification::Ldac)
            .unwrap_or(unknown(frame)),
        (0x05, 2) => Notification::PromptVolume(body[1]),
        (0xD3, 2) => Notification::ShutdownTimerEnabled(body[1] != 0x00),
        (0xD3, 3) => Notification::ShutdownTimer { minutes: body[2] },
        (0xD7, 2) => Notification::AutoPowerOff(body[1] == 0x01),
        (0xC8, 7) => {
            let mut mac = [0u8; 6];
            mac.copy_from_slice(&body[1..7]);
            Notification::Mac(mac)
        }
        (0xC6, 4) => Notification::Firmware([body[1], body[2], body[3]]),
        (0xCC, 3) => match noise_mode(body[1]) {
            Some(mode) => Notification::Noise {
                mode,
                ambient_volume: body[2] as i8 - AMBIENT_VOLUME_OFFSET,
            },
            None => unknown(frame),
        },
        (0xC9, _) => Notification::Name(String::from_utf8_lossy(&body[1..]).into_owned()),
        (0xF0, 3) if body[1] == 0x0A => {
            Notification::ControlSettings(ControlSettings::from_mask(body[2]))
        }
        _ => unknown(frame),
    }
}

fn unknown(frame: &IncomingFrame) -> Notification {
    Notification::Unknown {
        head: frame.head,
        body: frame.body.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packet::{parse_hex, FrameDecoder};

    fn decode_one(hex: &str) -> Notification {
        let raw = parse_hex(hex).unwrap();
        let mut dec = FrameDecoder::new();
        let frame = dec.push(&raw).remove(0).unwrap();
        parse_notification(&frame)
    }

    #[test]
    fn parse_battery() {
        assert_eq!(decode_one("BB02D04D21F3"), Notification::Battery(77));
    }

    #[test]
    fn parse_game_mode_off() {
        assert_eq!(decode_one("BB02080020DE"), Notification::GameMode(false));
    }

    #[test]
    fn parse_noise_and_ambient() {
        assert_eq!(
            decode_one("BB03CC010621AA"),
            Notification::Noise {
                mode: NoiseMode::Normal,
                ambient_volume: 0,
            }
        );
    }

    #[test]
    fn parse_firmware() {
        assert_eq!(
            decode_one("BB04C603000221A3"),
            Notification::Firmware([0x03, 0x00, 0x02])
        );
    }

    #[test]
    fn parse_shutdown_timer_minutes() {
        assert_eq!(
            decode_one("BB03D3000521AF"),
            Notification::ShutdownTimer { minutes: 5 }
        );
    }

    #[test]
    fn parse_ack_cc() {
        match decode_one("CC02CA0121B2") {
            Notification::Ack { head, body } => {
                assert_eq!(head, 0xCC);
                assert_eq!(body, vec![0xCA, 0x01]);
            }
            other => panic!("{other:?}"),
        }
    }
}
