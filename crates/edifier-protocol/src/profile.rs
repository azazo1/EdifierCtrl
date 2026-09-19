use crate::constants::{BLE_SERVICE_UUID_SUFFIX, W800K_SERVICE_UUID};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Feature {
    Noise,
    AmbientSound,
    SoundEffect,
    ControlSettings,
    Ldac,
    GameMode,
    AutoPowerOff,
    PromptVolume,
    ShutdownTimer,
    Name,
    Playback,
}

impl Feature {
    pub const ALL: &[Feature] = &[
        Feature::Noise,
        Feature::AmbientSound,
        Feature::SoundEffect,
        Feature::ControlSettings,
        Feature::Ldac,
        Feature::GameMode,
        Feature::AutoPowerOff,
        Feature::PromptVolume,
        Feature::ShutdownTimer,
        Feature::Name,
        Feature::Playback,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Feature::Noise => "noise",
            Feature::AmbientSound => "ambient_sound",
            Feature::SoundEffect => "sound_effect",
            Feature::ControlSettings => "control_settings",
            Feature::Ldac => "ldac",
            Feature::GameMode => "game_mode",
            Feature::AutoPowerOff => "auto_power_off",
            Feature::PromptVolume => "prompt_volume",
            Feature::ShutdownTimer => "shutdown_timer",
            Feature::Name => "name",
            Feature::Playback => "playback",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProfileId {
    Generic,
    W820Nb,
    W820NbDoubleGold,
    W200BtPlus,
}

impl ProfileId {
    pub fn as_str(self) -> &'static str {
        match self {
            ProfileId::Generic => "basedevice",
            ProfileId::W820Nb => "w820nb",
            ProfileId::W820NbDoubleGold => "w820nbdoublegold",
            ProfileId::W200BtPlus => "w200btplus",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "basedevice" | "generic" => Some(Self::Generic),
            "w820nb" => Some(Self::W820Nb),
            "w820nbdoublegold" => Some(Self::W820NbDoubleGold),
            "w200btplus" => Some(Self::W200BtPlus),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceProfile {
    pub id: ProfileId,
    pub display_name: &'static str,
    pub unique_service_uuid: Option<&'static str>,
    pub max_name_len: usize,
    pub hidden: &'static [Feature],
}

impl DeviceProfile {
    pub fn supports(&self, feature: Feature) -> bool {
        !self.hidden.contains(&feature)
    }

    pub fn all() -> &'static [DeviceProfile] {
        PROFILES
    }

    pub fn by_id(id: ProfileId) -> &'static DeviceProfile {
        PROFILES
            .iter()
            .find(|p| p.id == id)
            .expect("内置档案必须覆盖全部 ProfileId")
    }

    pub fn by_key(key: &str) -> Option<&'static DeviceProfile> {
        ProfileId::parse(key).map(Self::by_id)
    }

    pub fn by_service_uuid(uuid: &str) -> Option<&'static DeviceProfile> {
        let lower = uuid.trim_matches(|c| c == '{' || c == '}').to_ascii_lowercase();
        PROFILES.iter().find(|p| {
            p.unique_service_uuid
                .map(|u| u.eq_ignore_ascii_case(&lower))
                .unwrap_or(false)
        })
    }

    pub fn looks_like_edifier_ble_service(uuid: &str) -> bool {
        let lower = uuid.to_ascii_lowercase();
        lower.contains(BLE_SERVICE_UUID_SUFFIX) || lower.eq_ignore_ascii_case(W800K_SERVICE_UUID)
    }

    pub fn check_name(&self, name: &str) -> Result<(), crate::error::ProtocolError> {
        if name.len() > self.max_name_len {
            Err(crate::error::ProtocolError::NameTooLong(self.max_name_len))
        } else {
            Ok(())
        }
    }
}

const PROFILES: &[DeviceProfile] = &[
    DeviceProfile {
        id: ProfileId::Generic,
        display_name: "Generic Device",
        unique_service_uuid: None,
        max_name_len: 24,
        hidden: &[],
    },
    DeviceProfile {
        id: ProfileId::W820Nb,
        display_name: "W820NB",
        unique_service_uuid: Some("48093801-1a48-11e9-ab14-d663bd873d93"),
        max_name_len: 24,
        hidden: &[Feature::SoundEffect, Feature::ControlSettings],
    },
    DeviceProfile {
        id: ProfileId::W820NbDoubleGold,
        display_name: "W820NB Double Gold",
        unique_service_uuid: Some("48097901-1a48-11e9-ab14-d663bd873d93"),
        max_name_len: 30,
        hidden: &[Feature::AutoPowerOff],
    },
    DeviceProfile {
        id: ProfileId::W200BtPlus,
        display_name: "W200BT Plus",
        unique_service_uuid: Some("48092801-1a48-11e9-ab14-d663bd873d93"),
        max_name_len: 24,
        hidden: &[
            Feature::Noise,
            Feature::AmbientSound,
            Feature::ControlSettings,
            Feature::Ldac,
            Feature::GameMode,
        ],
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn w820nb_hides_eq() {
        let p = DeviceProfile::by_id(ProfileId::W820Nb);
        assert!(!p.supports(Feature::SoundEffect));
        assert!(p.supports(Feature::Noise));
    }

    #[test]
    fn identify_by_uuid() {
        let p = DeviceProfile::by_service_uuid("{48097901-1A48-11E9-AB14-D663BD873D93}").unwrap();
        assert_eq!(p.id, ProfileId::W820NbDoubleGold);
    }
}
