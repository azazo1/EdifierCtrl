use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::command::Command;
use crate::error::ProtocolError;
use crate::packet::{parse_hex, to_hex};

pub const SETTINGS_FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettingsCommand {
    pub cmd: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub priority: u8,
}

fn is_zero(v: &u8) -> bool {
    *v == 0
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettingsFile {
    #[serde(default = "default_version")]
    pub version: u32,
    pub name: String,
    pub commands: Vec<SettingsCommand>,
}

fn default_version() -> u32 {
    0
}

impl SettingsFile {
    pub fn from_commands(profile_key: &str, commands: &[Command]) -> Result<Self, ProtocolError> {
        let mut items = Vec::new();
        for cmd in commands {
            let body = cmd.to_body()?;
            items.push(SettingsCommand {
                cmd: to_hex(&body),
                name: Some(cmd.label().to_string()),
                priority: cmd.export_priority(),
            });
        }
        Ok(Self {
            version: SETTINGS_FORMAT_VERSION,
            name: profile_key.to_string(),
            commands: items,
        })
    }

    pub fn parse_json(bytes: &[u8]) -> Result<Self, ProtocolError> {
        let value: Value =
            serde_json::from_slice(bytes).map_err(|e| ProtocolError::Json(e.to_string()))?;
        migrate(value)
    }

    pub fn to_json(&self) -> Result<String, ProtocolError> {
        serde_json::to_string_pretty(self).map_err(|e| ProtocolError::Json(e.to_string()))
    }

    pub fn command_bodies(&self) -> Result<Vec<Vec<u8>>, ProtocolError> {
        let mut grouped: [Vec<&SettingsCommand>; 3] = [Vec::new(), Vec::new(), Vec::new()];
        for item in &self.commands {
            let p = item.priority.min(2) as usize;
            grouped[p].push(item);
        }
        let mut out = Vec::new();
        for bucket in grouped {
            for item in bucket {
                if item.cmd.is_empty() {
                    continue;
                }
                out.push(parse_hex(&item.cmd)?);
            }
        }
        Ok(out)
    }
}

fn migrate(value: Value) -> Result<SettingsFile, ProtocolError> {
    let obj: Map<String, Value> = match value {
        Value::Object(map) => map,
        _ => return Err(ProtocolError::SettingsNotObject),
    };
    let version = obj
        .get("version")
        .and_then(Value::as_u64)
        .unwrap_or(0) as u32;
    if version > SETTINGS_FORMAT_VERSION {
        return Err(ProtocolError::SettingsVersion(version));
    }
    if !obj.get("commands").map(Value::is_array).unwrap_or(false) {
        return Err(ProtocolError::SettingsMissingCommands);
    }
    let mut with_version = obj;
    with_version.insert("version".into(), Value::from(SETTINGS_FORMAT_VERSION));
    if !with_version.contains_key("name") {
        with_version.insert("name".into(), Value::from("basedevice"));
    }
    serde_json::from_value(Value::Object(with_version))
        .map_err(|e| ProtocolError::Json(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::{LdacMode, NoiseMode};

    #[test]
    fn import_medifier_file_without_version() {
        let json = r#"{"name":"w820nbdoublegold","commands":[{"cmd":"c101","name":"Noise Reduction","priority":1},{"cmd":"4902","name":"LDAC","priority":2},{"cmd":"0901"}]}"#;
        let file = SettingsFile::parse_json(json.as_bytes()).unwrap();
        assert_eq!(file.version, SETTINGS_FORMAT_VERSION);
        let bodies = file.command_bodies().unwrap();
        assert_eq!(bodies[0], vec![0x09, 0x01]);
        assert_eq!(bodies[1], vec![0xC1, 0x01]);
        assert_eq!(bodies[2], vec![0x49, 0x02]);
    }

    #[test]
    fn export_roundtrip_priority() {
        let file = SettingsFile::from_commands(
            "w820nb",
            &[
                Command::SetNoiseMode(NoiseMode::Reduction),
                Command::SetLdac(LdacMode::Rate96k),
                Command::SetGameMode(true),
            ],
        )
        .unwrap();
        let bodies = file.command_bodies().unwrap();
        assert_eq!(bodies[0], vec![0x09, 0x01]);
        assert_eq!(bodies[1], vec![0xC1, 0x02]);
        assert_eq!(bodies[2], vec![0x49, 0x02]);
    }
}
