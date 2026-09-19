use edifier_protocol::{
    encode_tx, to_hex, DeviceProfile, Feature, ProtocolError, SettingsFile,
};
use edifier_session::readout_plan;
use serde::Serialize;

use crate::command_json::CommandJson;

#[derive(Serialize)]
pub struct ProfileJson {
    pub id: String,
    pub display_name: String,
    pub max_name_len: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unique_service_uuid: Option<String>,
    pub features: Vec<&'static str>,
}

impl ProfileJson {
    pub fn from_profile(p: &DeviceProfile) -> Self {
        Self {
            id: p.id.as_str().to_string(),
            display_name: p.display_name.to_string(),
            max_name_len: p.max_name_len,
            unique_service_uuid: p.unique_service_uuid.map(str::to_string),
            features: Feature::ALL
                .iter()
                .filter(|f| p.supports(**f))
                .map(|f| f.as_str())
                .collect(),
        }
    }
}

pub fn profiles_json() -> Result<String, String> {
    let list: Vec<ProfileJson> = DeviceProfile::all()
        .iter()
        .map(ProfileJson::from_profile)
        .collect();
    serde_json::to_string(&list).map_err(|e| e.to_string())
}

#[derive(Serialize)]
pub struct PlannedCommand {
    pub label: String,
    pub body: String,
    pub hex: String,
    pub op: CommandJson,
}

pub fn readout_json(profile_key: &str) -> Result<String, String> {
    let profile = DeviceProfile::by_key(profile_key).ok_or("未知机型档案")?;
    let mut out = Vec::new();
    for cmd in readout_plan(profile) {
        let body = cmd.to_body().map_err(|e| e.to_string())?;
        let frame = encode_tx(&body).map_err(|e| e.to_string())?;
        out.push(PlannedCommand {
            label: cmd.label().to_string(),
            body: to_hex(&body),
            hex: to_hex(&frame),
            op: CommandJson::from_command(&cmd).map_err(|e| e.to_string())?,
        });
    }
    serde_json::to_string(&out).map_err(|e| e.to_string())
}

#[derive(Serialize)]
pub struct SettingsView {
    pub version: u32,
    pub name: String,
    pub bodies: Vec<String>,
}

pub fn settings_parse_json(input: &str) -> Result<String, String> {
    let file = SettingsFile::parse_json(input.as_bytes()).map_err(err_str)?;
    let bodies = file
        .command_bodies()
        .map_err(err_str)?
        .into_iter()
        .map(|b| to_hex(&b))
        .collect();
    serde_json::to_string(&SettingsView {
        version: file.version,
        name: file.name,
        bodies,
    })
    .map_err(|e| e.to_string())
}

fn err_str(e: ProtocolError) -> String {
    e.to_string()
}
