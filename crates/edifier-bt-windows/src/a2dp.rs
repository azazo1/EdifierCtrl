//! Win32 开关本机到耳机的 A2DP 服务.
//! AudioPlaybackConnection.TryCreateFromId 在本机即使用 DispatcherQueue 也会访问冲突.

use std::mem::size_of;

use edifier_runtime::TransportError;
use tracing::{info, warn};
use windows::core::GUID;
use windows::Win32::Devices::Bluetooth::{
    BluetoothFindDeviceClose, BluetoothFindFirstDevice, BluetoothFindFirstRadio,
    BluetoothFindRadioClose, BluetoothGetDeviceInfo, BluetoothSetServiceState, BLUETOOTH_ADDRESS,
    BLUETOOTH_ADDRESS_0, BLUETOOTH_DEVICE_INFO, BLUETOOTH_DEVICE_SEARCH_PARAMS,
    BLUETOOTH_FIND_RADIO_PARAMS, BLUETOOTH_SERVICE_DISABLE, BLUETOOTH_SERVICE_ENABLE,
};
use windows::Win32::Foundation::{CloseHandle, BOOL, HANDLE, TRUE};

use crate::com::parse_addr;
use crate::com::win_err;

const A2DP_SINK: GUID = GUID::from_u128(0x0000_110b_0000_1000_8000_0080_5f9b_34fb);
const A2DP_SOURCE: GUID = GUID::from_u128(0x0000_110a_0000_1000_8000_0080_5f9b_34fb);
const AVRCP: GUID = GUID::from_u128(0x0000_110e_0000_1000_8000_0080_5f9b_34fb);
const HANDSFREE: GUID = GUID::from_u128(0x0000_111e_0000_1000_8000_0080_5f9b_34fb);

pub fn set_a2dp(address: &str, enable: bool) -> Result<(), TransportError> {
    let addr = parse_addr(address)?;
    let flag = if enable {
        BLUETOOTH_SERVICE_ENABLE
    } else {
        BLUETOOTH_SERVICE_DISABLE
    };
    with_radio(|radio| {
        let info = find_device(radio, addr)?;
        let mut last = 0u32;
        for (name, guid) in [
            ("a2dp-sink", A2DP_SINK),
            ("a2dp-source", A2DP_SOURCE),
            ("avrcp", AVRCP),
            ("handsfree", HANDSFREE),
        ] {
            let rc = unsafe { BluetoothSetServiceState(radio, &info, &guid, flag) };
            if rc == 0 {
                info!(
                    target: "edifier_bt_windows",
                    address,
                    enable,
                    service = name,
                    "已设置蓝牙音频服务"
                );
                return Ok(());
            }
            last = rc;
            warn!(
                target: "edifier_bt_windows",
                address,
                service = name,
                win32 = rc,
                "BluetoothSetServiceState 失败"
            );
        }
        Err(TransportError::Connect(format!(
            "BluetoothSetServiceState 全部失败 last={last}"
        )))
    })
}

pub fn a2dp_connected(address: &str) -> Result<bool, TransportError> {
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
