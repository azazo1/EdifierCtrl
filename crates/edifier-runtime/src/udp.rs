use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};

use async_trait::async_trait;
use socket2::{Domain, Protocol, Socket, Type};
use tokio::net::UdpSocket;
use tracing::info;

use crate::net::{GroupNet, GROUP_PORT};
use crate::transport::TransportError;

pub const GROUP_MCAST: Ipv4Addr = Ipv4Addr::new(239, 18, 70, 17);

pub struct UdpGroupNet {
    sock: UdpSocket,
}

impl UdpGroupNet {
    pub async fn bind() -> Result<Self, TransportError> {
        let std_sock = bind_reuse()?;
        std_sock
            .set_nonblocking(true)
            .map_err(|e| TransportError::Unavailable(e.to_string()))?;
        let sock = UdpSocket::from_std(std_sock)
            .map_err(|e| TransportError::Unavailable(e.to_string()))?;
        sock.join_multicast_v4(GROUP_MCAST, Ipv4Addr::UNSPECIFIED)
            .map_err(|e| TransportError::Unavailable(e.to_string()))?;
        let _ = sock.set_multicast_loop_v4(true);
        info!(
            target: "edifier_runtime",
            port = GROUP_PORT,
            mcast = %GROUP_MCAST,
            "已加入组播"
        );
        Ok(Self { sock })
    }
}

fn bind_reuse() -> Result<std::net::UdpSocket, TransportError> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))
        .map_err(|e| TransportError::Unavailable(e.to_string()))?;
    socket
        .set_reuse_address(true)
        .map_err(|e| TransportError::Unavailable(e.to_string()))?;
    let addr = SocketAddr::from(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, GROUP_PORT));
    socket
        .bind(&socket2::SockAddr::from(addr))
        .map_err(|e| TransportError::Unavailable(e.to_string()))?;
    Ok(socket.into())
}

#[async_trait]
impl GroupNet for UdpGroupNet {
    async fn send(&self, bytes: &[u8]) -> Result<(), TransportError> {
        self.sock
            .send_to(bytes, (GROUP_MCAST, GROUP_PORT))
            .await
            .map_err(|e| TransportError::Write(e.to_string()))?;
        Ok(())
    }

    async fn recv(&self) -> Result<Vec<u8>, TransportError> {
        let mut buf = vec![0u8; 4096];
        let (n, _) = self
            .sock
            .recv_from(&mut buf)
            .await
            .map_err(|e| TransportError::Unavailable(e.to_string()))?;
        buf.truncate(n);
        Ok(buf)
    }
}
