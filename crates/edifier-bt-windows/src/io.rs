use std::sync::mpsc as std_mpsc;

use edifier_runtime::TransportError;
use tokio::sync::mpsc;
use tracing::{debug, warn};
use windows::Networking::Sockets::StreamSocket;
use windows::Storage::Streams::{DataReader, DataWriter, InputStreamOptions};

use crate::com::{spawn_mta, win_err, Mta};

impl Mta<StreamSocket> {
    pub fn close(self) {
        let _ = self.0.Close();
    }
}

pub struct BytePipe {
    pub writes: std_mpsc::Sender<Vec<u8>>,
    pub reads: mpsc::UnboundedReceiver<Vec<u8>>,
    pub socket: Mta<StreamSocket>,
}

pub fn spawn_socket(socket: Mta<StreamSocket>) -> Result<BytePipe, TransportError> {
    let socket = socket.0;
    let keep = Mta(socket.clone());
    let output = socket.OutputStream().map_err(win_err)?;
    let input = socket.InputStream().map_err(win_err)?;
    let (wtx, wrx) = std_mpsc::channel::<Vec<u8>>();
    let (rtx, rrx) = mpsc::unbounded_channel();

    spawn_mta("edifier-rfcomm-w", move || {
        let writer = match DataWriter::CreateDataWriter(&output) {
            Ok(w) => w,
            Err(err) => {
                warn!(target: "edifier_bt_windows", %err, "创建 DataWriter 失败");
                return;
            }
        };
        while let Ok(bytes) = wrx.recv() {
            if writer.WriteBytes(&bytes).is_err() {
                break;
            }
            match writer.StoreAsync().and_then(|op| op.get()) {
                Ok(_) => debug!(target: "edifier_bt_windows", len = bytes.len(), "RFCOMM 已写"),
                Err(err) => {
                    warn!(target: "edifier_bt_windows", %err, "RFCOMM 写失败");
                    break;
                }
            }
        }
    })?;

    spawn_mta("edifier-rfcomm-r", move || {
        let _keep = socket;
        let reader = match DataReader::CreateDataReader(&input) {
            Ok(r) => r,
            Err(err) => {
                warn!(target: "edifier_bt_windows", %err, "创建 DataReader 失败");
                return;
            }
        };
        if reader
            .SetInputStreamOptions(InputStreamOptions::Partial)
            .is_err()
        {
            return;
        }
        loop {
            match reader.LoadAsync(512).and_then(|op| op.get()) {
                Ok(0) => break,
                Err(err) => {
                    debug!(target: "edifier_bt_windows", %err, "RFCOMM 读结束");
                    break;
                }
                Ok(n) => {
                    let mut buf = vec![0u8; n as usize];
                    if reader.ReadBytes(&mut buf).is_err() {
                        break;
                    }
                    if rtx.send(buf).is_err() {
                        break;
                    }
                }
            }
        }
    })?;

    Ok(BytePipe {
        writes: wtx,
        reads: rrx,
        socket: keep,
    })
}
