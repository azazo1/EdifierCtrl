use edifier_protocol::RFCOMM_SERVICE_UUID;
use edifier_runtime::{LinkKind, ScanResult, TransportError};
use tracing::info;
use windows::Devices::Bluetooth::BluetoothDevice;
use windows::Devices::Bluetooth::Rfcomm::{RfcommDeviceService, RfcommServiceId};
use windows::Devices::Enumeration::DeviceInformation;
use windows::Networking::Sockets::StreamSocket;

use crate::com::{format_addr, parse_addr, win_err, Mta, RFCOMM_GUID};

pub fn scan() -> Result<Vec<ScanResult>, TransportError> {
    let sid = RfcommServiceId::FromUuid(RFCOMM_GUID).map_err(win_err)?;
    let selector = RfcommDeviceService::GetDeviceSelector(&sid).map_err(win_err)?;
    let col = DeviceInformation::FindAllAsyncAqsFilter(&selector)
        .map_err(win_err)?
        .get()
        .map_err(win_err)?;
    let n = col.Size().map_err(win_err)?;
    let mut out = Vec::new();
    for i in 0..n {
        let info = col.GetAt(i).map_err(win_err)?;
        let id = info.Id().map_err(win_err)?;
        let svc = RfcommDeviceService::FromIdAsync(&id)
            .map_err(win_err)?
            .get()
            .map_err(win_err)?;
        let dev = svc.Device().map_err(win_err)?;
        let addr = format_addr(dev.BluetoothAddress().map_err(win_err)?);
        let name = dev.Name().map_err(win_err)?.to_string();
        info!(target: "edifier_bt_windows", %addr, %name, "扫描到 RFCOMM 耳机");
        out.push(ScanResult {
            address: addr,
            name,
            kind: LinkKind::Rfcomm,
            service_uuid: Some(RFCOMM_SERVICE_UUID.into()),
        });
    }
    Ok(out)
}

pub fn connect(address: &str) -> Result<Mta<StreamSocket>, TransportError> {
    let addr = parse_addr(address)?;
    let device = BluetoothDevice::FromBluetoothAddressAsync(addr)
        .map_err(win_err)?
        .get()
        .map_err(win_err)?;
    let sid = RfcommServiceId::FromUuid(RFCOMM_GUID).map_err(win_err)?;
    let result = device
        .GetRfcommServicesForIdAsync(&sid)
        .map_err(win_err)?
        .get()
        .map_err(win_err)?;
    let services = result.Services().map_err(win_err)?;
    if services.Size().map_err(win_err)? == 0 {
        return Err(TransportError::NotFound(format!(
            "设备 {address} 没有漫步者 RFCOMM 服务, 请确认本机已用经典蓝牙配对"
        )));
    }
    let svc = services.GetAt(0).map_err(win_err)?;
    let socket = StreamSocket::new().map_err(win_err)?;
    socket
        .ConnectAsync(
            &svc.ConnectionHostName().map_err(win_err)?,
            &svc.ConnectionServiceName().map_err(win_err)?,
        )
        .map_err(win_err)?
        .get()
        .map_err(win_err)?;
    info!(target: "edifier_bt_windows", address, "RFCOMM 已连接");
    Ok(Mta(socket))
}
