use edifier_runtime::{AudioState, TransportError};
use windows::core::{GUID, HSTRING, Interface};
use windows::Devices::Bluetooth::BluetoothDevice;
use windows::Devices::Enumeration::{DeviceInformation, DeviceInformationKind};
use windows::Foundation::{Collections::IIterable, IPropertyValue};
use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_ContainerId;
use windows::Win32::Media::Audio::{
    eRender, IMMDeviceEnumerator, MMDeviceEnumerator, DEVICE_STATE, DEVICE_STATE_ACTIVE,
    DEVICE_STATE_NOTPRESENT, DEVICE_STATE_UNPLUGGED, DEVICE_STATEMASK_ALL,
};
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL, STGM_READ};
use windows::Win32::System::Com::StructuredStorage::PropVariantToGUID;

use crate::{a2dp, com::{parse_addr, win_err}};
use super::observed::{self, EndpointState};

pub(super) fn audio_state(address: &str) -> Result<AudioState, TransportError> {
    // ACL 断开可以排除音频连接. ACL 仍在时, 关掉服务或端点消失都不能确认释放.
    if !a2dp::acl_connected(address)? {
        return Ok(AudioState::Disconnected);
    }
    let containers = device_containers(address)?;
    if containers.is_empty() {
        return Ok(observed::audio_state([]));
    }
    let enumerator: IMMDeviceEnumerator = unsafe {
        CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
    }.map_err(win_err)?;
    let endpoints = unsafe {
        enumerator.EnumAudioEndpoints(eRender, DEVICE_STATE(DEVICE_STATEMASK_ALL))
    }.map_err(win_err)?;
    let mut states = Vec::new();
    for index in 0..unsafe { endpoints.GetCount() }.map_err(win_err)? {
        let endpoint = unsafe { endpoints.Item(index) }.map_err(win_err)?;
        let props = unsafe { endpoint.OpenPropertyStore(STGM_READ) }.map_err(win_err)?;
        let value = unsafe { props.GetValue(&PKEY_Device_ContainerId) }.map_err(win_err)?;
        let Ok(container) = (unsafe { PropVariantToGUID(&value) }) else {
            // 无法关联的端点可能属于目标耳机, 不能据此确认释放.
            states.push(EndpointState::Unknown);
            continue;
        };
        if !containers.contains(&container) {
            continue;
        }
        let state = unsafe { endpoint.GetState() }.map_err(win_err)?;
        states.push(if state == DEVICE_STATE_ACTIVE {
            EndpointState::Active
        } else if state == DEVICE_STATE_UNPLUGGED || state == DEVICE_STATE_NOTPRESENT {
            EndpointState::Disconnected
        } else {
            // DISABLED 只是控制面板隐藏端点, 已打开的音频流仍可能运行.
            EndpointState::Unknown
        });
    }
    Ok(observed::audio_state(states))
}

fn device_containers(address: &str) -> Result<Vec<GUID>, TransportError> {
    // 持有状态只查询已配对的目标. 按地址发现的默认 selector 会主动 inquiry, 超出交接期限.
    let paired = BluetoothDevice::GetDeviceSelectorFromPairingState(true).map_err(win_err)?;
    let addr = parse_addr(address)?;
    let selector = HSTRING::from(format!(
        "{paired} AND System.DeviceInterface.Bluetooth.DeviceAddress:=\"{addr:012x}\""
    ));
    // Bluetooth selector 返回 AEP, 其容器属性不同于普通 PnP 设备属性.
    let key = HSTRING::from("System.Devices.Aep.ContainerId");
    let properties: IIterable<HSTRING> = vec![key.clone()].try_into().map_err(win_err)?;
    let devices = DeviceInformation::FindAllAsyncWithKindAqsFilterAndAdditionalProperties(
        &selector, &properties, DeviceInformationKind::AssociationEndpoint,
    ).map_err(win_err)?.get().map_err(win_err)?;
    let mut containers = Vec::new();
    for index in 0..devices.Size().map_err(win_err)? {
        let device = devices.GetAt(index).map_err(win_err)?;
        let container = device.Properties().map_err(win_err)?.Lookup(&key)
            .and_then(|value| value.cast::<IPropertyValue>())
            .and_then(|property| property.GetGuid());
        // 驱动尚未生成容器时属性可能为 null, 保留 Unknown 而非将空 WinRT 对象当异常.
        if let Ok(container) = container {
            if container != GUID::zeroed() && !containers.contains(&container) {
                containers.push(container);
            }
        }
    }
    Ok(containers)
}
