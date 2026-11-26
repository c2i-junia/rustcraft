//! The status probe: how many players a world has, asked on its status port.

use std::{
    future::Future,
    net::{Ipv4Addr, Ipv6Addr, SocketAddr},
    pin::Pin,
    time::Duration,
};

use rustcraft_protocol::status::{StatusReply, StatusRequest, STATUS_PACKET_LEN};
use tokio::net::UdpSocket;

/// The number of players, or nothing when the world did not answer.
pub type ProbeFuture<'a> = Pin<Box<dyn Future<Output = Option<u32>> + Send + 'a>>;

/// What asks the worlds for their status.
pub trait StatusProbe: Send + Sync {
    /// Asks the world whose status port is `port`.
    fn players(&self, port: u16) -> ProbeFuture<'_>;
}

/// Asks over UDP, with the status exchange of `rustcraft_protocol::status`, at `host`.
pub struct UdpProbe {
    pub host: String,
    pub timeout: Duration,
}

impl UdpProbe {
    async fn ask(&self, port: u16) -> std::io::Result<Option<u32>> {
        let Some(address) = tokio::net::lookup_host((self.host.as_str(), port))
            .await?
            .next()
        else {
            return Ok(None);
        };
        let local: SocketAddr = match address {
            SocketAddr::V4(_) => (Ipv4Addr::UNSPECIFIED, 0).into(),
            SocketAddr::V6(_) => (Ipv6Addr::UNSPECIFIED, 0).into(),
        };
        let socket = UdpSocket::bind(local).await?;
        socket.connect(address).await?;
        let nonce = rand::random();
        socket.send(&StatusRequest { nonce }.encode()).await?;
        let mut buffer = [0; STATUS_PACKET_LEN + 1];
        let reply = async {
            loop {
                let read = socket.recv(&mut buffer).await?;
                match StatusReply::decode(&buffer[..read]) {
                    Some(reply) if reply.nonce == nonce => return Ok(Some(reply.players)),
                    _ => continue,
                }
            }
        };
        tokio::time::timeout(self.timeout, reply)
            .await
            .unwrap_or(Ok(None))
    }
}

impl StatusProbe for UdpProbe {
    fn players(&self, port: u16) -> ProbeFuture<'_> {
        Box::pin(async move { self.ask(port).await.ok().flatten() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_server_that_answers_tells_its_players_and_one_that_does_not_nothing() {
        let server = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let port = server.local_addr().unwrap().port();
        tokio::spawn(async move {
            let mut buffer = [0; 64];
            loop {
                let (read, from) = server.recv_from(&mut buffer).await.unwrap();
                let request = StatusRequest::decode(&buffer[..read]).unwrap();
                // A stray reply first, which the probe must not take for the answer
                let stray = StatusReply {
                    nonce: request.nonce.wrapping_add(1),
                    protocol: 1,
                    players: 9,
                    max_players: 10,
                };
                server.send_to(&stray.encode(), from).await.unwrap();
                let reply = StatusReply {
                    players: 3,
                    nonce: request.nonce,
                    ..stray
                };
                server.send_to(&reply.encode(), from).await.unwrap();
            }
        });
        let probe = UdpProbe {
            host: "127.0.0.1".into(),
            timeout: Duration::from_millis(500),
        };
        assert_eq!(probe.players(port).await, Some(3));

        let silent = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let silent_port = silent.local_addr().unwrap().port();
        assert_eq!(probe.players(silent_port).await, None);
        drop(silent);
        assert_eq!(probe.players(silent_port).await, None);
    }
}
