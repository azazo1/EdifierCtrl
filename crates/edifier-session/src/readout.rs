use edifier_protocol::{Command, DeviceProfile, Feature};

/// 按机型可见功能生成读取顺序, 间隔由 runtime 使用 COMMAND_GAP_MS.
pub fn readout_plan(profile: &DeviceProfile) -> Vec<Command> {
    let mut cmds = vec![
        Command::QueryBattery,
        Command::QueryMac,
        Command::QueryFirmware,
    ];
    if profile.supports(Feature::AmbientSound) || profile.supports(Feature::Noise) {
        cmds.push(Command::QueryNoise);
    }
    if profile.supports(Feature::Name) {
        cmds.push(Command::QueryName);
    }
    if profile.supports(Feature::SoundEffect) {
        cmds.push(Command::QuerySoundEffect);
    }
    if profile.supports(Feature::GameMode) {
        cmds.push(Command::QueryGameMode);
    }
    if profile.supports(Feature::ControlSettings) {
        cmds.push(Command::QueryControlSettings);
    }
    if profile.supports(Feature::Ldac) {
        cmds.push(Command::QueryLdac);
    }
    if profile.supports(Feature::PromptVolume) {
        cmds.push(Command::QueryPromptVolume);
    }
    if profile.supports(Feature::ShutdownTimer) {
        cmds.push(Command::QueryShutdownTimer);
    }
    if profile.supports(Feature::AutoPowerOff) {
        cmds.push(Command::QueryAutoPowerOff);
    }
    cmds
}

#[cfg(test)]
mod tests {
    use super::*;
    use edifier_protocol::ProfileId;

    #[test]
    fn w200bt_skips_noise_and_ldac() {
        let plan = readout_plan(DeviceProfile::by_id(ProfileId::W200BtPlus));
        assert!(!plan.iter().any(|c| matches!(c, Command::QueryNoise)));
        assert!(!plan.iter().any(|c| matches!(c, Command::QueryLdac)));
        assert!(plan.iter().any(|c| matches!(c, Command::QueryBattery)));
    }
}
