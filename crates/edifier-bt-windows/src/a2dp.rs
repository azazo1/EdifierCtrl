//! 通过公开 Win32 API 开关远端音频服务, 服务启用不代表音频已连接.

use std::mem::size_of;

use edifier_runtime::TransportError;
use tracing::{info, warn};
use windows::core::GUID;
use windows::Win32::Devices::Bluetooth::{
    BluetoothEnumerateInstalledServices, BluetoothFindDeviceClose, BluetoothFindFirstDevice, BluetoothFindFirstRadio,
    BluetoothFindRadioClose, BluetoothGetDeviceInfo, BluetoothSetServiceState, BLUETOOTH_ADDRESS,
    BLUETOOTH_ADDRESS_0, BLUETOOTH_DEVICE_INFO, BLUETOOTH_DEVICE_SEARCH_PARAMS,
    BLUETOOTH_FIND_RADIO_PARAMS, BLUETOOTH_SERVICE_DISABLE, BLUETOOTH_SERVICE_ENABLE,
};
use windows::Win32::Foundation::{CloseHandle, BOOL, HANDLE, TRUE};

use crate::audio::services::{AudioService, ServiceControl};
use crate::com::parse_addr;
use crate::com::win_err;

const A2DP_SINK: GUID = GUID::from_u128(0x0000_110b_0000_1000_8000_0080_5f9b_34fb);
const HANDSFREE: GUID = GUID::from_u128(0x0000_111e_0000_1000_8000_0080_5f9b_34fb);
const HEADSET: GUID = GUID::from_u128(0x0000_1108_0000_1000_8000_0080_5f9b_34fb);

const AUDIO_SERVICES: [(AudioService, GUID); 3] = [
    (AudioService::A2dp, A2DP_SINK),
    (AudioService::Handsfree, HANDSFREE),
    (AudioService::Headset, HEADSET),
];

pub struct Win32Services;

impl ServiceControl for Win32Services {
    fn enabled(&self, address: &str) -> Result<Vec<AudioService>, TransportError> {
        enabled_audio_services(address)
    }

    fn set(&self, address: &str, service: AudioService, enable: bool) -> Result<(), TransportError> {
        let addr = parse_addr(address)?;
        let guid = AUDIO_SERVICES.iter().find(|(candidate, _)| *candidate == service)
            .expect("音频服务枚举完整").1;
        let flag = if enable { BLUETOOTH_SERVICE_ENABLE } else { BLUETOOTH_SERVICE_DISABLE };
        with_radio(|radio| {
            let info = find_device(radio, addr)?;
            let rc = unsafe { BluetoothSetServiceState(radio, &info, &guid, flag) };
            // 有效 flags 下 E_INVALIDARG 表示服务已经处于请求的启用状态.
            if rc == 0 || rc == windows::Win32::Foundation::E_INVALIDARG.0 as u32 {
                info!(target: "edifier_bt_windows", address, enable, ?service, "已请求蓝牙音频服务状态");
                Ok(())
            } else {
                warn!(target: "edifier_bt_windows", address, ?service, win32 = rc, "BluetoothSetServiceState 失败");
                Err(TransportError::Connect(format!("BluetoothSetServiceState {service:?} win32={rc}")))
            }
        })
    }
}

pub fn enabled_audio_services(address: &str) -> Result<Vec<AudioService>, TransportError> {
    let addr = parse_addr(address)?;
    with_radio(|radio| {
        let info = find_device(radio, addr)?;
        let mut count = 0;
        let rc = unsafe { BluetoothEnumerateInstalledServices(radio, &info, &mut count, None) };
        let more = windows::Win32::Foundation::ERROR_MORE_DATA.0;
        if rc != 0 && rc != more {
            return Err(TransportError::Unavailable(format!("枚举已启用音频服务 win32={rc}")));
        }
        for _ in 0..3 {
            if count == 0 {
                return Ok(Vec::new());
            }
            let mut guids = vec![GUID::zeroed(); count as usize];
            let rc = unsafe {
                BluetoothEnumerateInstalledServices(radio, &info, &mut count, Some(guids.as_mut_ptr()))
            };
            if rc == 0 {
                guids.truncate(count as usize);
                return Ok(AUDIO_SERVICES.iter().filter_map(|(service, guid)| {
                    guids.contains(guid).then_some(*service)
                }).collect());
            }
            if rc != more {
                return Err(TransportError::Unavailable(format!("枚举已启用音频服务 win32={rc}")));
            }
        }
        Err(TransportError::Unavailable("音频服务列表持续变化".into()))
    })
}

pub fn acl_connected(address: &str) -> Result<bool, TransportError> {
    let addr = parse_addr(address)?;
    with_radio(|radio| {
        let info = find_device(radio, addr)?;
        Ok(info.fConnected.as_bool())
    })
}

fn find_device(radio: HANDLE, addr: u64) -> Result<BLUETOOTH_DEVICE_INFO, TransportError> {
    if let Ok(info) = device_info(radio, addr) {
        return Ok(info);
    }
    let params = BLUETOOTH_DEVICE_SEARCH_PARAMS {
        dwSize: size_of::<BLUETOOTH_DEVICE_SEARCH_PARAMS>() as u32,
        fReturnAuthenticated: TRUE,
        fReturnRemembered: TRUE,
        fReturnUnknown: BOOL::default(),
        fReturnConnected: TRUE,
        fIssueInquiry: BOOL::default(),
        cTimeoutMultiplier: 0,
        hRadio: radio,
    };
    let mut info = BLUETOOTH_DEVICE_INFO {
        dwSize: size_of::<BLUETOOTH_DEVICE_INFO>() as u32,
        ..Default::default()
    };
    let find = unsafe { BluetoothFindFirstDevice(&params, &mut info) }.map_err(win_err)?;
    loop {
        let got = unsafe { info.Address.Anonymous.ullLong };
        if got == addr {
            let _ = unsafe { BluetoothFindDeviceClose(find) };
            return Ok(info);
        }
        info = BLUETOOTH_DEVICE_INFO {
            dwSize: size_of::<BLUETOOTH_DEVICE_INFO>() as u32,
            ..Default::default()
        };
        if unsafe {
            windows::Win32::Devices::Bluetooth::BluetoothFindNextDevice(find, &mut info)
        }
        .is_err()
        {
            break;
        }
    }
    let _ = unsafe { BluetoothFindDeviceClose(find) };
    Err(TransportError::NotFound(format!(
        "Win32 找不到蓝牙设备 {addr:#x}"
    )))
}

fn device_info(radio: HANDLE, addr: u64) -> Result<BLUETOOTH_DEVICE_INFO, TransportError> {
    let mut info = BLUETOOTH_DEVICE_INFO {
        dwSize: size_of::<BLUETOOTH_DEVICE_INFO>() as u32,
        Address: BLUETOOTH_ADDRESS {
            Anonymous: BLUETOOTH_ADDRESS_0 { ullLong: addr },
        },
        ..Default::default()
    };
    let rc = unsafe { BluetoothGetDeviceInfo(radio, &mut info) };
    if rc != 0 {
        return Err(TransportError::NotFound(format!(
            "BluetoothGetDeviceInfo win32={rc}"
        )));
    }
    Ok(info)
}

fn with_radio<T>(f: impl FnOnce(HANDLE) -> Result<T, TransportError>) -> Result<T, TransportError> {
    let params = BLUETOOTH_FIND_RADIO_PARAMS {
        dwSize: size_of::<BLUETOOTH_FIND_RADIO_PARAMS>() as u32,
    };
    let mut radio = HANDLE::default();
    let find = unsafe { BluetoothFindFirstRadio(&params, &mut radio) }.map_err(win_err)?;
    if radio.is_invalid() {
        let _ = unsafe { BluetoothFindRadioClose(find) };
        return Err(TransportError::Unavailable("没有蓝牙适配器".into()));
    }
    let result = f(radio);
    unsafe {
        let _ = CloseHandle(radio);
        let _ = BluetoothFindRadioClose(find);
    }
    result
}
