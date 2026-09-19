use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use dbus::arg::PropMap;
use dbus::nonblock::{Proxy, SyncConnection};
use dbus::nonblock::stdintf::org_freedesktop_dbus::ObjectManager;
use dbus::Path;
use edifier_runtime::{AudioState, TransportError};
use tracing::info;

use super::observed::{self, Transport, A2DP_SINK};

const BLUEZ: &str = "org.bluez";
const DEVICE: &str = "org.bluez.Device1";
const TRANSPORT: &str = "org.bluez.MediaTransport1";
const CALL_TIMEOUT: Duration = Duration::from_secs(8);
type Objects = HashMap<Path<'static>, HashMap<String, PropMap>>;

pub(super) async fn audio_state(address: &str) -> Result<AudioState, TransportError> {
    let address = parse_address(address)?;
    with_bus(move |connection| async move {
        let objects = snapshot(&connection).await?;
        let devices = device_paths(&objects, &address)?;
        Ok(observe(&objects, &devices))
    }).await
}

pub(super) async fn set_connected(address: &str, connect: bool) -> Result<(), TransportError> {
    let address = parse_address(address)?;
    with_bus(move |connection| async move {
        let objects = snapshot(&connection).await?;
        let devices = device_paths(&objects, &address)?;
        let state = observe(&objects, &devices);
        if (connect && state == AudioState::Connected)
            || (!connect && state == AudioState::Disconnected) {
            return Ok(());
        }
        let method = if connect { "ConnectProfile" } else { "Disconnect" };
        let mut failure = None;
        for path in devices {
            let proxy = Proxy::new(BLUEZ, path, CALL_TIMEOUT, connection.clone());
            let result: Result<(), dbus::Error> = if connect {
                proxy.method_call(DEVICE, method, (A2DP_SINK,)).await
            } else {
                // BlueZ 无通用 HFP 状态, 释放整个设备才能同时排除 HFP/SCO 占用.
                proxy.method_call(DEVICE, method, ()).await
            };
            match result {
                Ok(()) => {
                    info!(target: "edifier_bt_linux", address, method, "已请求蓝牙音频状态");
                    if connect {
                        return Ok(());
                    }
                }
                Err(err) if !connect && err.name() == Some("org.bluez.Error.NotConnected") => {}
                Err(err) => failure = Some(TransportError::Connect(err.to_string())),
            }
        }
        match failure {
            Some(err) => Err(err),
            None => Ok(()),
        }
    }).await
}

async fn with_bus<T, F, Fut>(operation: F) -> Result<T, TransportError>
where
    F: FnOnce(Arc<SyncConnection>) -> Fut,
    Fut: Future<Output = Result<T, TransportError>>,
{
    let (resource, connection) = tokio::task::spawn_blocking(dbus_tokio::connection::new_system_sync)
        .await
        .map_err(|err| TransportError::Unavailable(err.to_string()))?
        .map_err(bus_error)?;
    // 资源 future 与调用共享生命周期, 取消调用时不会遗留 D-Bus 后台任务.
    tokio::select! {
        result = operation(connection) => result,
        error = resource => Err(TransportError::Unavailable(format!("BlueZ D-Bus: {error}"))),
    }
}

async fn snapshot(connection: &Arc<SyncConnection>) -> Result<Objects, TransportError> {
    Proxy::new(BLUEZ, "/", CALL_TIMEOUT, connection.clone())
        .get_managed_objects().await.map_err(bus_error)
}

fn parse_address(address: &str) -> Result<String, TransportError> {
    address.parse::<bluer::Address>()
        .map(|address| address.to_string())
        .map_err(|err| TransportError::NotFound(err.to_string()))
}

fn device_paths(objects: &Objects, address: &str) -> Result<Vec<Path<'static>>, TransportError> {
    let mut devices: Vec<_> = objects.iter().filter_map(|(path, interfaces)| {
        let props = interfaces.get(DEVICE)?;
        text(props, "Address").filter(|value| value.eq_ignore_ascii_case(address))?;
        Some(path.clone())
    }).collect();
    devices.sort();
    if devices.is_empty() {
        return Err(TransportError::NotFound(format!("BlueZ 没有设备 {address}")));
    }
    Ok(devices)
}

fn observe(objects: &Objects, devices: &[Path<'static>]) -> AudioState {
    let transports = objects.iter().filter_map(|(path, interfaces)| {
        let props = interfaces.get(TRANSPORT)?;
        let Some(device) = text(props, "Device") else {
            // Device 是必填属性, 缺失时仅按对象归属识别不完整的目标 transport.
            return devices.iter().any(|device| path.starts_with(&format!("{device}/")))
                .then_some(Transport { uuid: None, state: None });
        };
        if !devices.iter().any(|path| &**path == device) {
            return None;
        }
        Some(Transport { uuid: text(props, "UUID"), state: text(props, "State") })
    });
    let all_acls_disconnected = devices.iter().all(|path| {
        objects.get(path).and_then(|interfaces| interfaces.get(DEVICE))
            .and_then(|props| dbus::arg::prop_cast::<bool>(props, "Connected")) == Some(&false)
    });
    observed::audio_state(transports, all_acls_disconnected)
}

fn text<'a>(props: &'a PropMap, name: &str) -> Option<&'a str> {
    props.get(name)?.0.as_str()
}

fn bus_error(error: dbus::Error) -> TransportError {
    TransportError::Unavailable(format!("BlueZ D-Bus: {error}"))
}

#[cfg(test)]
mod tests {
    use dbus::arg::{RefArg, Variant};
    use super::*;

    fn props(values: Vec<(&str, Box<dyn RefArg>)>) -> PropMap {
        values.into_iter().map(|(key, value)| (key.into(), Variant(value))).collect()
    }

    #[test]
    fn acl_alone_does_not_prove_audio_and_other_devices_do_not_count() {
        let device = Path::new("/org/bluez/hci0/dev_11_22_33_44_55_66").unwrap();
        let other = Path::new("/org/bluez/hci0/dev_AA_BB_CC_DD_EE_FF").unwrap();
        let mut objects = Objects::new();
        objects.insert(device.clone(), HashMap::from([(DEVICE.into(), props(vec![
            ("Address", Box::new("11:22:33:44:55:66".to_string())),
            ("Connected", Box::new(true)),
        ]))]));
        objects.insert(Path::new(format!("{other}/fd0")).unwrap(), HashMap::from([
            (TRANSPORT.into(), props(vec![
                ("Device", Box::new(other)),
                ("UUID", Box::new(A2DP_SINK.to_string())),
                ("State", Box::new("active".to_string())),
            ])),
        ]));
        let paths = device_paths(&objects, "11:22:33:44:55:66").unwrap();
        assert_eq!(observe(&objects, &paths), AudioState::Unknown);
        objects.get_mut(&device).unwrap().get_mut(DEVICE).unwrap()
            .insert("Connected".into(), Variant(Box::new(false)));
        assert_eq!(observe(&objects, &paths), AudioState::Disconnected);
        assert!(device_paths(&objects, "00:00:00:00:00:00").is_err());
        objects.insert(Path::new(format!("{device}/fd0")).unwrap(), HashMap::from([
            (TRANSPORT.into(), props(vec![
                ("Device", Box::new(device)),
                ("UUID", Box::new(A2DP_SINK.to_string())),
                ("State", Box::new("idle".to_string())),
            ])),
        ]));
        assert_eq!(observe(&objects, &paths), AudioState::Connected);
    }
}
