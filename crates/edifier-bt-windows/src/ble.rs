use edifier_protocol::{DeviceProfile, BLE_RX_UUID_HINT, BLE_TX_UUID_HINT, SPECIAL_BLE_RX_UUIDS, SPECIAL_BLE_TX_UUIDS};
use edifier_runtime::{LinkKind, ScanResult, TransportError};
use tokio::sync::mpsc;
use tracing::{info, warn};
use windows::Devices::Bluetooth::GenericAttributeProfile::{
    GattCharacteristic, GattCharacteristicProperties, GattClientCharacteristicConfigurationDescriptorValue,
    GattCommunicationStatus, GattDeviceService, GattValueChangedEventArgs,
};
use windows::Devices::Bluetooth::{BluetoothCacheMode, BluetoothLEDevice};
use windows::Devices::Enumeration::DeviceInformation;
use windows::Foundation::TypedEventHandler;
use windows::Storage::Streams::DataWriter;

use std::sync::mpsc as std_mpsc;

use crate::com::{format_addr, guid_text, parse_addr, spawn_mta, win_err};

pub struct BleLink {
    pub writes: GattCharacteristic,
    pub reads: Option<mpsc::UnboundedReceiver<Vec<u8>>>,
    _device: BluetoothLEDevice,
    _service: GattDeviceService,
    _notify: GattCharacteristic,
}

unsafe impl Send for BleLink {}
unsafe impl Sync for BleLink {}

pub fn scan() -> Result<Vec<ScanResult>, TransportError> {
    let selector = BluetoothLEDevice::GetDeviceSelector().map_err(win_err)?;
    let col = DeviceInformation::FindAllAsyncAqsFilter(&selector)
        .map_err(win_err)?
        .get()
        .map_err(win_err)?;
    let n = col.Size().map_err(win_err)?;
    let mut out = Vec::new();
    for i in 0..n {
        let info = col.GetAt(i).map_err(win_err)?;
        let id = info.Id().map_err(win_err)?;
        let Ok(dev) = BluetoothLEDevice::FromIdAsync(&id).and_then(|op| op.get()) else {
            continue;
        };
        let Ok(result) = dev.GetGattServicesAsync().and_then(|op| op.get()) else {
            continue;
        };
        let Ok(services) = result.Services() else {
            continue;
        };
        let size = services.Size().unwrap_or(0);
        let mut uuid = None;
        for j in 0..size {
            let Ok(svc) = services.GetAt(j) else {
                continue;
            };
            let Ok(g) = svc.Uuid() else {
                continue;
            };
            let text = guid_text(g);
            if DeviceProfile::looks_like_edifier_ble_service(&text) {
                uuid = Some(text.trim_matches(|c| c == '{' || c == '}').to_ascii_lowercase());
                break;
            }
        }
        if uuid.is_none() {
            continue;
        }
        let addr = format_addr(dev.BluetoothAddress().map_err(win_err)?);
        let name = dev.Name().map_err(win_err)?.to_string();
        info!(target: "edifier_bt_windows", %addr, %name, "扫描到 BLE 耳机");
        out.push(ScanResult {
            address: addr,
            name,
            kind: LinkKind::Ble,
            service_uuid: uuid,
        });
    }
    Ok(out)
}

pub fn connect(address: &str) -> Result<BleLink, TransportError> {
    let addr = parse_addr(address)?;
    let device = BluetoothLEDevice::FromBluetoothAddressAsync(addr)
        .map_err(win_err)?
        .get()
        .map_err(win_err)?;
    let result = device
        .GetGattServicesWithCacheModeAsync(BluetoothCacheMode::Uncached)
        .map_err(win_err)?
        .get()
        .map_err(win_err)?;
    let services = result.Services().map_err(win_err)?;
    let mut chosen: Option<GattDeviceService> = None;
    for i in 0..services.Size().map_err(win_err)? {
        let svc = services.GetAt(i).map_err(win_err)?;
        let text = guid_text(svc.Uuid().map_err(win_err)?);
        if DeviceProfile::looks_like_edifier_ble_service(&text) {
            chosen = Some(svc);
            break;
        }
    }
    let service = chosen.ok_or_else(|| {
        TransportError::NotFound(format!(
            "设备 {address} 没有漫步者 BLE 服务, Windows 随机 MAC 时请改用 RFCOMM"
        ))
    })?;
    let chars = service
        .GetCharacteristicsAsync()
        .map_err(win_err)?
        .get()
        .map_err(win_err)?
        .Characteristics()
        .map_err(win_err)?;
    let mut tx = None;
    let mut rx = None;
    for i in 0..chars.Size().map_err(win_err)? {
        let ch = chars.GetAt(i).map_err(win_err)?;
        let text = guid_text(ch.Uuid().map_err(win_err)?).to_ascii_lowercase();
        if is_tx(&text) {
            tx = Some(ch);
        } else if is_rx(&text) {
            rx = Some(ch);
        }
    }
    let tx = tx.ok_or_else(|| TransportError::NotFound("未找到 BLE 写特征".into()))?;
    let rx = rx.ok_or_else(|| TransportError::NotFound("未找到 BLE 通知特征".into()))?;
    let props = rx.CharacteristicProperties().map_err(win_err)?;
    if props.contains(GattCharacteristicProperties::Notify) {
        let status = rx
            .WriteClientCharacteristicConfigurationDescriptorAsync(
                GattClientCharacteristicConfigurationDescriptorValue::Notify,
            )
            .map_err(win_err)?
            .get()
            .map_err(win_err)?;
        if status != GattCommunicationStatus::Success {
            warn!(target: "edifier_bt_windows", ?status, "打开 BLE notify 未成功");
        }
    }
    let (rtx, rrx) = mpsc::unbounded_channel();
    rx.ValueChanged(&TypedEventHandler::new(
        move |_ch: &Option<GattCharacteristic>, args: &Option<GattValueChangedEventArgs>| {
            let Some(args) = args else {
                return Ok(());
            };
            let value = args.CharacteristicValue()?;
            let len = value.Length()?;
            let reader = windows::Storage::Streams::DataReader::FromBuffer(&value)?;
            let mut buf = vec![0u8; len as usize];
            reader.ReadBytes(&mut buf)?;
            let _ = rtx.send(buf);
            Ok(())
        },
    ))
    .map_err(win_err)?;
    info!(target: "edifier_bt_windows", address, "BLE 已连接");
    Ok(BleLink {
        writes: tx,
        reads: Some(rrx),
        _device: device,
        _service: service,
        _notify: rx,
    })
}

pub fn spawn_writer(ch: GattCharacteristic) -> Result<std_mpsc::Sender<Vec<u8>>, TransportError> {
    let (tx, rx) = std_mpsc::channel::<Vec<u8>>();
    spawn_mta("edifier-ble-w", move || {
        while let Ok(bytes) = rx.recv() {
            if write_gatt(&ch, &bytes).is_err() {
                break;
            }
        }
    })?;
    Ok(tx)
}

pub fn write_gatt(ch: &GattCharacteristic, bytes: &[u8]) -> Result<(), TransportError> {
    let writer = DataWriter::new().map_err(win_err)?;
    writer.WriteBytes(bytes).map_err(win_err)?;
    let buf = writer.DetachBuffer().map_err(win_err)?;
    let status = ch
        .WriteValueAsync(&buf)
        .map_err(win_err)?
        .get()
        .map_err(win_err)?;
    if status != GattCommunicationStatus::Success {
        return Err(TransportError::Write(format!("BLE 写入状态 {status:?}")));
    }
    Ok(())
}

fn is_tx(uuid: &str) -> bool {
    uuid.contains(BLE_TX_UUID_HINT) || SPECIAL_BLE_TX_UUIDS.iter().any(|u| uuid.contains(u))
}

fn is_rx(uuid: &str) -> bool {
    uuid.contains(BLE_RX_UUID_HINT) || SPECIAL_BLE_RX_UUIDS.iter().any(|u| uuid.contains(u))
}


