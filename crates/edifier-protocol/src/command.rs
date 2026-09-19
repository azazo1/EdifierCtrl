use crate::constants::AMBIENT_VOLUME_OFFSET;
use crate::error::ProtocolError;
use crate::packet::encode_tx;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoiseMode {
    Normal,
    Reduction,
    Ambient,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoundEffect {
    Normal,
    Pop,
    Classical,
    Rock,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LdacMode {
    Off,
    Rate48k,
    Rate96k,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Playback {
    Play,
    Pause,
    VolumeUp,
    VolumeDown,
    Next,
    Previous,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ControlSettings {
    pub normal: bool,
    pub reduction: bool,
    pub ambient: bool,
}

impl ControlSettings {
    pub fn mask(self) -> u8 {
        let mut v = 0u8;
        if self.normal {
            v |= 1;
        }
        if self.reduction {
            v |= 2;
        }
        if self.ambient {
            v |= 4;
        }
        v
    }

    pub fn from_mask(v: u8) -> Self {
        Self {
            normal: v & 1 != 0,
            reduction: v & 2 != 0,
            ambient: v & 4 != 0,
        }
    }
}

/// 主机发给耳机的命令.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    SetNoiseMode(NoiseMode),
    SetAmbientVolume(i8),
    SetSoundEffect(SoundEffect),
    SetControlSettings(ControlSettings),
    SetLdac(LdacMode),
    SetGameMode(bool),
    SetPromptVolume(u8),
    DisableShutdownTimer,
    SetShutdownTimer { minutes: u8 },
    PowerOff,
    DisconnectHost,
    RePair,
    FactoryReset,
    SetName(String),
    Playback(Playback),
    SetAutoPowerOff(bool),
    QueryBattery,
    QueryMac,
    QueryFirmware,
    QueryNoise,
    QueryName,
    QuerySoundEffect,
    QueryGameMode,
    QueryControlSettings,
    QueryLdac,
    QueryPromptVolume,
    QueryShutdownTimer,
    QueryAutoPowerOff,
    QueryFingerprint,
    QueryPlayback,
    Raw(Vec<u8>),
}

impl Command {
    pub fn is_destructive(&self) -> bool {
        matches!(
            self,
            Command::PowerOff
                | Command::DisconnectHost
                | Command::RePair
                | Command::FactoryReset
        )
    }

    /// 导出设置时的优先级, 数字越大越后发.
    pub fn export_priority(&self) -> u8 {
        match self {
            Command::SetNoiseMode(_)
            | Command::SetAmbientVolume(_)
            | Command::DisableShutdownTimer
            | Command::SetShutdownTimer { .. } => 1,
            Command::SetLdac(_) => 2,
            _ => 0,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Command::SetNoiseMode(_) => "Noise Reduction",
            Command::SetAmbientVolume(_) => "Ambient Sound",
            Command::SetSoundEffect(_) => "Sound Effect",
            Command::SetControlSettings(_) => "Control Settings",
            Command::SetLdac(_) => "LDAC",
            Command::SetGameMode(_) => "Game Mode",
            Command::SetPromptVolume(_) => "Prompt Volume",
            Command::DisableShutdownTimer => "Shutdown Timer Enabled",
            Command::SetShutdownTimer { .. } => "Shutdown Timer",
            Command::PowerOff => "Power Off",
            Command::DisconnectHost => "Disconnect",
            Command::RePair => "Re-pair",
            Command::FactoryReset => "Reset",
            Command::SetName(_) => "Name",
            Command::Playback(_) => "Playback",
            Command::SetAutoPowerOff(_) => "Auto Poweroff",
            Command::QueryBattery => "Battery",
            Command::QueryMac => "MAC",
            Command::QueryFirmware => "Firmware",
            Command::QueryNoise => "Noise",
            Command::QueryName => "Name Query",
            Command::QuerySoundEffect => "Sound Effect Query",
            Command::QueryGameMode => "Game Mode Query",
            Command::QueryControlSettings => "Control Settings Query",
            Command::QueryLdac => "LDAC Query",
            Command::QueryPromptVolume => "Prompt Volume Query",
            Command::QueryShutdownTimer => "Shutdown Timer Query",
            Command::QueryAutoPowerOff => "Auto Poweroff Query",
            Command::QueryFingerprint => "Fingerprint",
            Command::QueryPlayback => "Playback Query",
            Command::Raw(_) => "Raw",
        }
    }

    pub fn to_body(&self) -> Result<Vec<u8>, ProtocolError> {
        Ok(match self {
            Command::SetNoiseMode(mode) => vec![
                0xC1,
                match mode {
                    NoiseMode::Normal => 0x01,
                    NoiseMode::Reduction => 0x02,
                    NoiseMode::Ambient => 0x03,
                },
            ],
            Command::SetAmbientVolume(vol) => {
                if *vol < -3 || *vol > 3 {
                    return Err(ProtocolError::AmbientVolumeRange);
                }
                vec![0xC1, 0x03, (AMBIENT_VOLUME_OFFSET + vol) as u8]
            }
            Command::SetSoundEffect(fx) => vec![
                0xC4,
                match fx {
                    SoundEffect::Normal => 0x00,
                    SoundEffect::Pop => 0x01,
                    SoundEffect::Classical => 0x02,
                    SoundEffect::Rock => 0x03,
                },
            ],
            Command::SetControlSettings(cs) => vec![0xF1, 0x0A, cs.mask()],
            Command::SetLdac(mode) => vec![
                0x49,
                match mode {
                    LdacMode::Off => 0x00,
                    LdacMode::Rate48k => 0x01,
                    LdacMode::Rate96k => 0x02,
                },
            ],
            Command::SetGameMode(on) => vec![0x09, if *on { 0x01 } else { 0x00 }],
            Command::SetPromptVolume(v) => vec![0x06, *v],
            Command::DisableShutdownTimer => vec![0xD2],
            Command::SetShutdownTimer { minutes } => vec![0xD1, 0x00, *minutes],
            Command::PowerOff => vec![0xCE],
            Command::DisconnectHost => vec![0xCD],
            Command::RePair => vec![0xCF],
            Command::FactoryReset => vec![0x07],
            Command::SetName(name) => {
                let mut body = vec![0xCA];
                body.extend_from_slice(name.as_bytes());
                body
            }
            Command::Playback(p) => vec![
                0xC2,
                match p {
                    Playback::Play => 0x00,
                    Playback::Pause => 0x01,
                    Playback::VolumeUp => 0x02,
                    Playback::VolumeDown => 0x03,
                    Playback::Next => 0x04,
                    Playback::Previous => 0x05,
                },
            ],
            Command::SetAutoPowerOff(on) => vec![0xD6, if *on { 0x01 } else { 0x00 }],
            Command::QueryBattery => vec![0xD0],
            Command::QueryMac => vec![0xC8],
            Command::QueryFirmware => vec![0xC6],
            Command::QueryNoise => vec![0xCC],
            Command::QueryName => vec![0xC9],
            Command::QuerySoundEffect => vec![0xD5],
            Command::QueryGameMode => vec![0x08],
            Command::QueryControlSettings => vec![0xF0, 0x0A],
            Command::QueryLdac => vec![0x48],
            Command::QueryPromptVolume => vec![0x05],
            Command::QueryShutdownTimer => vec![0xD3],
            Command::QueryAutoPowerOff => vec![0xD7],
            Command::QueryFingerprint => vec![0xD8],
            Command::QueryPlayback => vec![0xC3],
            Command::Raw(bytes) => bytes.clone(),
        })
    }

    pub fn to_frame(&self) -> Result<Vec<u8>, ProtocolError> {
        encode_tx(&self.to_body()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packet::to_hex;

    fn frame_hex(cmd: Command) -> String {
        to_hex(&cmd.to_frame().unwrap())
    }

    #[test]
    fn captures_match_known_commands() {
        assert_eq!(
            frame_hex(Command::SetNoiseMode(NoiseMode::Normal)),
            "AA02C1012187"
        );
        assert_eq!(
            frame_hex(Command::SetAmbientVolume(-3)),
            "AA03C10303218D"
        );
        assert_eq!(frame_hex(Command::SetGameMode(true)), "AA02090120CF");
        assert_eq!(frame_hex(Command::SetLdac(LdacMode::Rate96k)), "AA0249022110");
        assert_eq!(frame_hex(Command::PowerOff), "AA01CE2192");
        assert_eq!(frame_hex(Command::FactoryReset), "AA010720CB");
        assert_eq!(
            frame_hex(Command::SetShutdownTimer { minutes: 60 }),
            "AA03D1003C21D3"
        );
        assert_eq!(
            frame_hex(Command::SetControlSettings(ControlSettings {
                normal: true,
                reduction: true,
                ambient: true,
            })),
            "AA03F10A0721C8"
        );
        assert_eq!(frame_hex(Command::QueryBattery), "AA01D02194");
        assert_eq!(frame_hex(Command::QueryControlSettings), "AA02F00A21BF");
    }
}
