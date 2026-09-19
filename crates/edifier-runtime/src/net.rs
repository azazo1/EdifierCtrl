use std::collections::VecDeque;
use std::sync::{Arc, Mutex as StdMutex};

use async_trait::async_trait;
use edifier_group::Envelope;
use serde::{Deserialize, Serialize};
use tokio::sync::Notify;

use crate::transport::TransportError;

pub const GROUP_PORT: u16 = 18721;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Datagram {
    pub from: String,
    pub envelope: Envelope,
}

#[async_trait]
pub trait GroupNet: Send + Sync {
    async fn send(&self, bytes: &[u8]) -> Result<(), TransportError>;
    async fn recv(&self) -> Result<Vec<u8>, TransportError>;
}

/// 进程内总线, 不回环给自己.
#[derive(Clone)]
pub struct LoopbackNet {
    id: usize,
    inner: Arc<StdMutex<LoopbackInner>>,
    notify: Arc<Notify>,
}

struct LoopbackInner {
    next_id: usize,
    inboxes: Vec<VecDeque<Vec<u8>>>,
}

impl LoopbackNet {
    pub fn pair() -> (Self, Self) {
        let inner = Arc::new(StdMutex::new(LoopbackInner {
            next_id: 0,
            inboxes: Vec::new(),
        }));
        let notify = Arc::new(Notify::new());
        (Self::attach(&inner, &notify), Self::attach(&inner, &notify))
    }

    fn attach(inner: &Arc<StdMutex<LoopbackInner>>, notify: &Arc<Notify>) -> Self {
        let mut g = inner.lock().expect("loopback");
        let id = g.next_id;
        g.next_id += 1;
        g.inboxes.push(VecDeque::new());
        Self {
            id,
            inner: inner.clone(),
            notify: notify.clone(),
        }
    }
}

#[async_trait]
impl GroupNet for LoopbackNet {
    async fn send(&self, bytes: &[u8]) -> Result<(), TransportError> {
        {
            let mut g = self.inner.lock().expect("loopback");
            for (i, inbox) in g.inboxes.iter_mut().enumerate() {
                if i != self.id {
                    inbox.push_back(bytes.to_vec());
                }
            }
        }
        self.notify.notify_waiters();
        Ok(())
    }

    async fn recv(&self) -> Result<Vec<u8>, TransportError> {
        loop {
            {
                let mut g = self.inner.lock().expect("loopback");
                if let Some(bytes) = g.inboxes[self.id].pop_front() {
                    return Ok(bytes);
                }
            }
            self.notify.notified().await;
        }
    }
}
