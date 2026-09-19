use edifier_protocol::{
    encode_tx, parse_hex, to_hex, Command, ControlSettings, LdacMode, NoiseMode, Playback,
    ProtocolError, SoundEffect,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum CommandJson {
    SetNoiseMode { mode: String },
    SetAmbientVolume { volume: i8 },
    SetSoundEffect { effect: String },
    SetControlSettings {
        normal: bool,
        reduction: bool,
        ambient: bool,
    },
    SetLdac { mode: String },
    SetGameMode { on: bool },
    SetPromptVolume { volume: u8 },
    DisableShutdownTimer,
    SetShutdownTimer { minutes: u8 },
    PowerOff,
    DisconnectHost,
    RePair,
    FactoryReset,
    SetName { name: String },
    Playback { action: String },
    SetAutoPowerOff { on: bool },
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
    Raw { hex: String },
}

impl CommandJson {
    pub fn from_command(cmd: &Command) -> Result<Self, ProtocolError> {
        Ok(match cmd {
            Command::SetNoiseMode(mode) => Self::SetNoiseMode {
                mode: noise_str(*mode).into(),
            },
            Command::SetAmbientVolume(v) => Self::SetAmbientVolume { volume: *v },
            Command::SetSoundEffect(fx) => Self::SetSoundEffect {
                effect: effect_str(*fx).into(),
            },
            Command::SetControlSettings(cs) => Self::SetControlSettings {
                normal: cs.normal,
                reduction: cs.reduction,
                ambient: cs.ambient,
            },
            Command::SetLdac(mode) => Self::SetLdac {
                mode: ldac_str(*mode).into(),
            },
            Command::SetGameMode(on) => Self::SetGameMode { on: *on },
            Command::SetPromptVolume(v) => Self::SetPromptVolume { volume: *v },
            Command::DisableShutdownTimer => Self::DisableShutdownTimer,
            Command::SetShutdownTimer { minutes } => Self::SetShutdownTimer { minutes: *minutes },
            Command::PowerOff => Self::PowerOff,
            Command::DisconnectHost => Self::DisconnectHost,
            Command::RePair => Self::RePair,
            Command::FactoryReset => Self::FactoryReset,
            Command::SetName(name) => Self::SetName { name: name.clone() },
            Command::Playback(p) => Self::Playback {
                action: playback_str(*p).into(),
            },
            Command::SetAutoPowerOff(on) => Self::SetAutoPowerOff { on: *on },
            Command::QueryBattery => Self::QueryBattery,
            Command::QueryMac => Self::QueryMac,
            Command::QueryFirmware => Self::QueryFirmware,
            Command::QueryNoise => Self::QueryNoise,
            Command::QueryName => Self::QueryName,
            Command::QuerySoundEffect => Self::QuerySoundEffect,
            Command::QueryGameMode => Self::QueryGameMode,
            Command::QueryControlSettings => Self::QueryControlSettings,
            Command::QueryLdac => Self::QueryLdac,
            Command::QueryPromptVolume => Self::QueryPromptVolume,
            Command::QueryShutdownTimer => Self::QueryShutdownTimer,
            Command::QueryAutoPowerOff => Self::QueryAutoPowerOff,
            Command::QueryFingerprint => Self::QueryFingerprint,
            Command::QueryPlayback => Self::QueryPlayback,
            Command::Raw(bytes) => Self::Raw {
                hex: to_hex(bytes),
            },
        })
    }

    pub fn to_command(&self) -> Result<Command, String> {
        Ok(match self {
            Self::SetNoiseMode { mode } => Command::SetNoiseMode(parse_noise(mode)?),
            Self::SetAmbientVolume { volume } => Command::SetAmbientVolume(*volume),
            Self::SetSoundEffect { effect } => Command::SetSoundEffect(parse_effect(effect)?),
            Self::SetControlSettings {
                normal,
                reduction,
                ambient,
            } => Command::SetControlSettings(ControlSettings {
                normal: *normal,
                reduction: *reduction,
                ambient: *ambient,
            }),
            Self::SetLdac { mode } => Command::SetLdac(parse_ldac(mode)?),
            Self::SetGameMode { on } => Command::SetGameMode(*on),
            Self::SetPromptVolume { volume } => Command::SetPromptVolume(*volume),
            Self::DisableShutdownTimer => Command::DisableShutdownTimer,
            Self::SetShutdownTimer { minutes } => Command::SetShutdownTimer { minutes: *minutes },
            Self::PowerOff => Command::PowerOff,
            Self::DisconnectHost => Command::DisconnectHost,
            Self::RePair => Command::RePair,
            Self::FactoryReset => Command::FactoryReset,
            Self::SetName { name } => Command::SetName(name.clone()),
            Self::Playback { action } => Command::Playback(parse_playback(action)?),
            Self::SetAutoPowerOff { on } => Command::SetAutoPowerOff(*on),
            Self::QueryBattery => Command::QueryBattery,
            Self::QueryMac => Command::QueryMac,
            Self::QueryFirmware => Command::QueryFirmware,
            Self::QueryNoise => Command::QueryNoise,
            Self::QueryName => Command::QueryName,
            Self::QuerySoundEffect => Command::QuerySoundEffect,
            Self::QueryGameMode => Command::QueryGameMode,
            Self::QueryControlSettings => Command::QueryControlSettings,
            Self::QueryLdac => Command::QueryLdac,
            Self::QueryPromptVolume => Command::QueryPromptVolume,
            Self::QueryShutdownTimer => Command::QueryShutdownTimer,
            Self::QueryAutoPowerOff => Command::QueryAutoPowerOff,
            Self::QueryFingerprint => Command::QueryFingerprint,
            Self::QueryPlayback => Command::QueryPlayback,
            Self::Raw { hex } => Command::Raw(parse_hex(hex).map_err(|e| e.to_string())?),
        })
    }
}

#[derive(Serialize)]
pub struct EncodedCommand {
    pub hex: String,
    pub body: String,
    pub destructive: bool,
    pub label: String,
    pub op: CommandJson,
}

pub fn encode_command_json(input: &str) -> Result<EncodedCommand, String> {
    let json: CommandJson = serde_json::from_str(input).map_err(|e| e.to_string())?;
    let cmd = json.to_command()?;
    let body = cmd.to_body().map_err(|e| e.to_string())?;
    let frame = encode_tx(&body).map_err(|e| e.to_string())?;
    Ok(EncodedCommand {
        hex: to_hex(&frame),
        body: to_hex(&body),
        destructive: cmd.is_destructive(),
        label: cmd.label().to_string(),
        op: json,
    })
}

fn noise_str(mode: NoiseMode) -> &'static str {
    match mode {
        NoiseMode::Normal => "normal",
        NoiseMode::Reduction => "reduction",
        NoiseMode::Ambient => "ambient",
    }
}

fn parse_noise(s: &str) -> Result<NoiseMode, String> {
    match s {
        "normal" => Ok(NoiseMode::Normal),
        "reduction" => Ok(NoiseMode::Reduction),
        "ambient" => Ok(NoiseMode::Ambient),
        _ => Err(format!("未知降噪模式 {s}")),
    }
}

fn effect_str(fx: SoundEffect) -> &'static str {
    match fx {
        SoundEffect::Normal => "normal",
        SoundEffect::Pop => "pop",
        SoundEffect::Classical => "classical",
        SoundEffect::Rock => "rock",
    }
}

fn parse_effect(s: &str) -> Result<SoundEffect, String> {
    match s {
        "normal" => Ok(SoundEffect::Normal),
        "pop" => Ok(SoundEffect::Pop),
        "classical" => Ok(SoundEffect::Classical),
        "rock" => Ok(SoundEffect::Rock),
        _ => Err(format!("未知音效 {s}")),
    }
}

fn ldac_str(mode: LdacMode) -> &'static str {
    match mode {
        LdacMode::Off => "off",
        LdacMode::Rate48k => "rate48k",
        LdacMode::Rate96k => "rate96k",
    }
}

fn parse_ldac(s: &str) -> Result<LdacMode, String> {
    match s {
        "off" => Ok(LdacMode::Off),
        "rate48k" | "48k" => Ok(LdacMode::Rate48k),
        "rate96k" | "96k" => Ok(LdacMode::Rate96k),
        _ => Err(format!("未知 LDAC 模式 {s}")),
    }
}

fn playback_str(p: Playback) -> &'static str {
    match p {
        Playback::Play => "play",
        Playback::Pause => "pause",
        Playback::VolumeUp => "volume_up",
        Playback::VolumeDown => "volume_down",
        Playback::Next => "next",
        Playback::Previous => "previous",
    }
}

fn parse_playback(s: &str) -> Result<Playback, String> {
    match s {
        "play" => Ok(Playback::Play),
        "pause" => Ok(Playback::Pause),
        "volume_up" => Ok(Playback::VolumeUp),
        "volume_down" => Ok(Playback::VolumeDown),
        "next" => Ok(Playback::Next),
        "previous" => Ok(Playback::Previous),
        _ => Err(format!("未知播放动作 {s}")),
    }
}
