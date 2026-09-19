use edifier_runtime::AudioState;

#[derive(Clone, Copy)]
pub(super) enum EndpointState {
    Active,
    Disconnected,
    Unknown,
}

pub(super) fn audio_state(
    states: impl IntoIterator<Item = EndpointState>,
    services_enabled: bool,
) -> AudioState {
    let mut found = false;
    let mut uncertain = false;
    for state in states {
        found = true;
        match state {
            EndpointState::Active => return AudioState::Connected,
            EndpointState::Disconnected => {}
            EndpointState::Unknown => uncertain = true,
        }
    }
    // 服务移除可能让端点完全消失, 此时需要系统服务枚举的独立负证据.
    if (found && !uncertain) || !services_enabled {
        AudioState::Disconnected
    } else {
        AudioState::Unknown
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_or_disabled_endpoint_cannot_confirm_release_with_services_enabled() {
        assert_eq!(audio_state([], true), AudioState::Unknown);
        assert_eq!(audio_state([EndpointState::Unknown], true), AudioState::Unknown);
        assert_eq!(
            audio_state([EndpointState::Disconnected, EndpointState::Unknown], true),
            AudioState::Unknown
        );
    }

    #[test]
    fn any_active_endpoint_keeps_audio_connected() {
        for services_enabled in [false, true] {
            assert_eq!(
                audio_state([EndpointState::Disconnected, EndpointState::Active], services_enabled),
                AudioState::Connected
            );
        }
    }

    #[test]
    fn all_observed_endpoints_must_be_disconnected() {
        assert_eq!(
            audio_state([EndpointState::Disconnected, EndpointState::Disconnected], true),
            AudioState::Disconnected
        );
    }

    #[test]
    fn removed_services_confirm_release_when_endpoints_disappear() {
        assert_eq!(audio_state([], false), AudioState::Disconnected);
    }
}
