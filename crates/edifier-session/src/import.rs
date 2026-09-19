use edifier_protocol::{Command, ProtocolError, SettingsFile};

/// 按 priority 0 -> 2 展开为待发送载荷.
pub fn import_plan(file: &SettingsFile) -> Result<Vec<Command>, ProtocolError> {
    Ok(file
        .command_bodies()?
        .into_iter()
        .map(Command::Raw)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn priority_order() {
        let json = br#"{"name":"x","commands":[{"cmd":"4900","priority":2},{"cmd":"0901"},{"cmd":"C102","priority":1}]}"#;
        let file = SettingsFile::parse_json(json).unwrap();
        let plan = import_plan(&file).unwrap();
        assert_eq!(plan[0], Command::Raw(vec![0x09, 0x01]));
        assert_eq!(plan[1], Command::Raw(vec![0xC1, 0x02]));
        assert_eq!(plan[2], Command::Raw(vec![0x49, 0x00]));
    }
}
