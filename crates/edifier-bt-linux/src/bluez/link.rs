use edifier_runtime::TransportError;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadHalf, WriteHalf};
use tokio::sync::{watch, Mutex};

/// 读写分离, 关闭通知可以取消正在等待的 I/O 和锁.
pub(super) struct Session<T> {
    reader: Mutex<ReadHalf<T>>,
    writer: Mutex<WriteHalf<T>>,
    closed: watch::Sender<bool>,
}

impl<T: AsyncRead + AsyncWrite + Unpin> Session<T> {
    pub(super) fn new(stream: T) -> Self {
        let (reader, writer) = tokio::io::split(stream);
        let (closed, _) = watch::channel(false);
        Self { reader: Mutex::new(reader), writer: Mutex::new(writer), closed }
    }

    pub(super) fn close(&self) {
        self.closed.send_replace(true);
    }

    pub(super) async fn write(&self, bytes: &[u8]) -> Result<(), TransportError> {
        let mut closed = self.closed.subscribe();
        if *closed.borrow() {
            return Err(TransportError::Closed);
        }
        tokio::select! {
            biased;
            _ = closed.changed() => Err(TransportError::Closed),
            result = async {
                self.writer.lock().await.write_all(bytes).await
                    .map_err(|err| TransportError::Write(err.to_string()))
            } => result,
        }
    }

    pub(super) async fn recv(&self) -> Result<Vec<u8>, TransportError> {
        let mut closed = self.closed.subscribe();
        if *closed.borrow() {
            return Err(TransportError::Closed);
        }
        let mut buffer = vec![0; 512];
        tokio::select! {
            biased;
            _ = closed.changed() => Err(TransportError::Closed),
            result = async {
                self.reader.lock().await.read(&mut buffer).await
                    .map_err(|err| TransportError::Connect(err.to_string()))
            } => {
                let size = result?;
                if size == 0 { return Err(TransportError::Closed); }
                buffer.truncate(size);
                Ok(buffer)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::time::timeout;
    use super::*;

    #[tokio::test]
    async fn waiting_recv_allows_write_and_local_close() {
        let (stream, mut peer) = tokio::io::duplex(64);
        let session = Arc::new(Session::new(stream));
        let receive = tokio::spawn({
            let session = session.clone();
            async move { session.recv().await }
        });
        tokio::task::yield_now().await;
        timeout(Duration::from_secs(1), session.write(&[1, 2, 3])).await.unwrap().unwrap();
        let mut bytes = [0; 3];
        timeout(Duration::from_secs(1), peer.read_exact(&mut bytes)).await.unwrap().unwrap();
        assert_eq!(bytes, [1, 2, 3]);
        session.close();
        assert!(matches!(timeout(Duration::from_secs(1), receive).await.unwrap().unwrap(),
            Err(TransportError::Closed)));
        assert!(matches!(session.write(&[4]).await, Err(TransportError::Closed)));
    }

    #[tokio::test]
    async fn local_close_cancels_backpressured_write() {
        let (stream, _peer) = tokio::io::duplex(1);
        let session = Arc::new(Session::new(stream));
        let write = tokio::spawn({
            let session = session.clone();
            async move { session.write(&[1, 2]).await }
        });
        tokio::task::yield_now().await;
        session.close();
        assert!(matches!(timeout(Duration::from_secs(1), write).await.unwrap().unwrap(),
            Err(TransportError::Closed)));
    }
}
