use edifier_runtime::AudioState;

#[derive(Clone, Copy)]
pub(super) enum EndpointState {
    Active,
    Disconnected,
    Unknown,
}

pub(super) fn audio_state(states: impl IntoIterator<Item = EndpointState>) -> AudioState {
    for state in states {
        match state {
            EndpointState::Active => return AudioState::Connected,
            EndpointState::Disconnected | EndpointState::Unknown => {}
        }
    }
    // 端点消失或未激活只能说明本机音频可能空闲, 不能证明系统蓝牙已断开.
    // 释放由 ACL 断开确认, 见 state.rs.
    AudioState::Unknown
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_or_unplugged_endpoints_cannot_confirm_release() {
        assert_eq!(audio_state([]), AudioState::Unknown);
        assert_eq!(audio_state([EndpointState::Unknown]), AudioState::Unknown);
        assert_eq!(audio_state([EndpointState::Disconnected]), AudioState::Unknown);
        assert_eq!(
            audio_state([EndpointState::Disconnected, EndpointState::Unknown]),
            AudioState::Unknown
        );
    }

    #[test]
    fn any_active_endpoint_keeps_audio_connected() {
        assert_eq!(
            audio_state([EndpointState::Disconnected, EndpointState::Active]),
            AudioState::Connected
        );
    }
}
