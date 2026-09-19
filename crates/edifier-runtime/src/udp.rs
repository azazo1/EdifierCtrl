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
        let iface = default_ipv4();
        let std_sock = bind_reuse(iface)?;
        let sock = UdpSocket::from_std(std_sock)
            .map_err(|e| TransportError::Unavailable(e.to_string()))?;
        info!(
            target: "edifier_runtime",
            port = GROUP_PORT,
            mcast = %GROUP_MCAST,
            iface = %iface,
            "已加入组播"
        );
        Ok(Self { sock })
    }
}

fn default_ipv4() -> Ipv4Addr {
    let probe = match std::net::UdpSocket::bind("0.0.0.0:0") {
        Ok(s) => s,
        Err(_) => return Ipv4Addr::UNSPECIFIED,
    };
    if probe.connect("8.8.8.8:80").is_err() {
        return Ipv4Addr::UNSPECIFIED;
    }
    match probe.local_addr() {
        Ok(SocketAddr::V4(addr)) => *addr.ip(),
        _ => Ipv4Addr::UNSPECIFIED,
    }
}

fn bind_reuse(iface: Ipv4Addr) -> Result<std::net::UdpSocket, TransportError> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))
        .map_err(|e| TransportError::Unavailable(e.to_string()))?;
    socket
        .set_reuse_address(true)
        .map_err(|e| TransportError::Unavailable(e.to_string()))?;
    let addr = SocketAddr::from(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, GROUP_PORT));
    socket
        .bind(&socket2::SockAddr::from(addr))
        .map_err(|e| TransportError::Unavailable(e.to_string()))?;
    let mut joined = false;
    for ifa in [iface, Ipv4Addr::UNSPECIFIED, Ipv4Addr::LOCALHOST] {
        if socket
            .join_multicast_v4(&GROUP_MCAST, &ifa)
            .is_ok()
        {
            joined = true;
        }
    }
    if !joined {
        return Err(TransportError::Unavailable("加入组播失败".into()));
    }
    let _ = socket.set_multicast_loop_v4(true);
    let _ = socket.set_multicast_ttl_v4(4);
    if iface != Ipv4Addr::UNSPECIFIED {
        let _ = socket.set_multicast_if_v4(&iface);
    }
    socket
        .set_nonblocking(true)
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
