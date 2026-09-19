use edifier_runtime::{AudioState, TransportError};
use windows::core::{GUID, HSTRING, Interface};
use windows::Devices::Bluetooth::BluetoothDevice;
use windows::Devices::Enumeration::DeviceInformation;
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
    // ACL 断开可以排除音频连接, ACL 连接本身不能证明音频已连接.
    if !a2dp::acl_connected(address)? {
        return Ok(AudioState::Disconnected);
    }
    let services_enabled = !a2dp::enabled_audio_services(address)?.is_empty();
    let containers = device_containers(address)?;
    if containers.is_empty() {
        return Ok(observed::audio_state([], services_enabled));
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
    Ok(observed::audio_state(states, services_enabled))
}

fn device_containers(address: &str) -> Result<Vec<GUID>, TransportError> {
    let selector = BluetoothDevice::GetDeviceSelectorFromBluetoothAddress(parse_addr(address)?)
        .map_err(win_err)?;
    let key = HSTRING::from("System.Devices.ContainerId");
    let properties: IIterable<HSTRING> = vec![key.clone()].try_into().map_err(win_err)?;
    let devices = DeviceInformation::FindAllAsyncAqsFilterAndAdditionalProperties(
        &selector, &properties,
    ).map_err(win_err)?.get().map_err(win_err)?;
    let mut containers = Vec::new();
    for index in 0..devices.Size().map_err(win_err)? {
        let device = devices.GetAt(index).map_err(win_err)?;
        let value = device.Properties().map_err(win_err)?.Lookup(&key).map_err(win_err)?;
        let property: IPropertyValue = value.cast().map_err(win_err)?;
        let container = property.GetGuid().map_err(win_err)?;
        if container != GUID::zeroed() {
            containers.push(container);
        }
    }
    Ok(containers)
}
