use edifier_runtime::AudioState;

pub(super) const A2DP_SINK: &str = "0000110b-0000-1000-8000-00805f9b34fb";
pub(super) const A2DP_SOURCE: &str = "0000110a-0000-1000-8000-00805f9b34fb";

pub(super) struct Transport<'a> {
    pub uuid: Option<&'a str>,
    pub state: Option<&'a str>,
}

pub(super) fn audio_state<'a>(
    transports: impl IntoIterator<Item = Transport<'a>>,
    all_acls_disconnected: bool,
) -> AudioState {
    for transport in transports {
        let Some(uuid) = transport.uuid else { continue; };
        if !uuid.eq_ignore_ascii_case(A2DP_SINK) && !uuid.eq_ignore_ascii_case(A2DP_SOURCE) {
            continue;
        }
        // idle 仅表示未播放, 已配置的 transport 仍占用 A2DP 连接.
        if matches!(transport.state, Some("idle" | "pending" | "active")) {
            return AudioState::Connected;
        }
    }
    // MediaTransport1 不涵盖 HFP/SCO. 没有 A2DP transport 时, ACL 必须断开才可确认释放.
    if all_acls_disconnected { AudioState::Disconnected } else { AudioState::Unknown }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_a2dp_transport_states_mean_connected() {
        for uuid in [A2DP_SINK, A2DP_SOURCE] {
            for state in ["idle", "pending", "active"] {
                assert_eq!(audio_state([Transport {
                    uuid: Some(uuid), state: Some(state),
                }], false), AudioState::Connected);
            }
        }
    }

    #[test]
    fn missing_and_unrecognized_properties_are_unknown() {
        for (uuid, state) in [(None, Some("active")), (Some(A2DP_SINK), None),
            (Some(A2DP_SINK), Some("unexpected"))] {
            assert_eq!(audio_state([Transport { uuid, state }], false), AudioState::Unknown);
        }
    }

    #[test]
    fn no_a2dp_transport_requires_acl_disconnection_to_confirm_release() {
        assert_eq!(audio_state([], true), AudioState::Disconnected);
        assert_eq!(audio_state([], false), AudioState::Unknown);
        assert_eq!(audio_state([Transport {
            uuid: Some("0000111e-0000-1000-8000-00805f9b34fb"), state: Some("active"),
        }], false), AudioState::Unknown);
    }
}
