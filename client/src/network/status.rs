//! The status of a server on the list: asked on its status port (see `shared::status`) from a
//! thread of its own, because resolving a host name and waiting for a reply block. The screen
//! only takes the result once it is there: the frame never waits on the network.

use std::io::ErrorKind;
use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use bevy::prelude::*;
use shared::status::{status_port, StatusReply, StatusRequest, STATUS_PACKET_LEN};
use shared::PROTOCOL_ID;

use super::address::{parse_server_address, resolve};

/// What the list knows of a server.
#[derive(Clone, Debug, PartialEq)]
pub enum ServerStatus {
    /// Asked, not answered yet.
    Pinging,
    /// Answered, and speaks the game's protocol.
    Online {
        players: u32,
        max_players: u32,
        /// From the send of the request answered to its reply.
        latency: Duration,
    },
    /// Answered, but speaks another protocol: it cannot be joined.
    Incompatible,
    /// Did not answer in time, or refused the request.
    Offline,
    /// Its host name does not resolve to an IPv4 address.
    UnknownHost,
    /// Its address, as typed, is not one.
    BadAddress,
}

/// How long a server has to answer before it is said offline.
pub const STATUS_TIMEOUT: Duration = Duration::from_secs(2);
/// How long a request waits for its reply before another one is sent, in case it was lost.
pub const RESEND_EVERY: Duration = Duration::from_millis(500);

/// Asks the server at `typed` (a `ServerItem::ip`) for its status. Blocks up to STATUS_TIMEOUT
/// plus the name resolution: call it from a thread.
pub fn query_status(typed: &str) -> ServerStatus {
    let Ok(address) = parse_server_address(typed) else {
        return ServerStatus::BadAddress;
    };
    let Ok(game) = resolve(&address) else {
        return ServerStatus::UnknownHost;
    };
    match status_port(game.port()) {
        Some(port) => query_status_at(SocketAddr::new(game.ip(), port), STATUS_TIMEOUT),
        None => ServerStatus::Offline,
    }
}

/// Asks the status port `status` itself: a connected socket, a new random nonce every
/// RESEND_EVERY, the latency from the send of the nonce answered.
pub fn query_status_at(status: SocketAddr, timeout: Duration) -> ServerStatus {
    let start = Instant::now();
    let Ok(socket) = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)) else {
        return ServerStatus::Offline;
    };
    // Connected: only datagrams from `status` are received, and a refusal is reported
    if socket.connect(status).is_err() {
        return ServerStatus::Offline;
    }
    let deadline = start + timeout;
    let mut sent: Vec<(u64, Instant)> = Vec::new();
    let mut next_send = start;
    // One byte more than a reply, so that a longer datagram is not cut down to one
    let mut buffer = [0; STATUS_PACKET_LEN + 1];
    loop {
        let now = Instant::now();
        if now >= deadline {
            return ServerStatus::Offline;
        }
        if now >= next_send {
            let nonce = rand::random();
            if socket.send(&StatusRequest { nonce }.encode()).is_err() {
                return ServerStatus::Offline;
            }
            sent.push((nonce, Instant::now()));
            next_send = now + RESEND_EVERY;
        }
        let wait = next_send
            .min(deadline)
            .saturating_duration_since(Instant::now());
        // A zero timeout is refused: wait at least a millisecond
        if socket
            .set_read_timeout(Some(wait.max(Duration::from_millis(1))))
            .is_err()
        {
            return ServerStatus::Offline;
        }
        match socket.recv(&mut buffer) {
            Ok(len) => {
                let Some(reply) = StatusReply::decode(&buffer[..len]) else {
                    continue;
                };
                let Some(&(_, sent_at)) = sent.iter().find(|(nonce, _)| *nonce == reply.nonce)
                else {
                    continue;
                };
                return if reply.protocol == PROTOCOL_ID {
                    ServerStatus::Online {
                        players: reply.players,
                        max_players: reply.max_players,
                        latency: sent_at.elapsed(),
                    }
                } else {
                    ServerStatus::Incompatible
                };
            }
            Err(err) if matches!(err.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            // Nothing listens there (ConnectionReset is how Windows says it), or the socket
            // failed: no answer will come
            Err(_) => return ServerStatus::Offline,
        }
    }
}

/// The number of bars, 1 to 4, that show `latency` on a card.
pub fn latency_bars(latency: Duration) -> u8 {
    match latency.as_millis() {
        0..=60 => 4,
        61..=120 => 3,
        121..=250 => 2,
        _ => 1,
    }
}

/// How the screen asks: `query_status`, replaced by tests with fixed answers.
#[derive(Resource, Clone, Copy)]
pub struct StatusProbe(pub fn(&str) -> ServerStatus);

impl Default for StatusProbe {
    fn default() -> Self {
        StatusProbe(query_status)
    }
}

/// The result of `f` run on its own thread, taken once. Dropping it abandons the result: a
/// probe replaced by a newer one can never land. The thread ends with `f`, and is not joined.
pub struct InFlight<T>(Arc<Mutex<Option<T>>>);

impl<T: Send + 'static> InFlight<T> {
    pub fn spawn(f: impl FnOnce() -> T + Send + 'static) -> Self {
        let slot = Arc::new(Mutex::new(None));
        let result = Arc::clone(&slot);
        thread::spawn(move || {
            let value = f();
            *result.lock().unwrap() = Some(value);
        });
        Self(slot)
    }

    /// An `InFlight` whose result is already there, for tests.
    #[cfg(test)]
    pub fn ready(value: T) -> Self {
        Self(Arc::new(Mutex::new(Some(value))))
    }

    /// The result once `f` has returned, the first time it is asked only. Never, if `f` panics.
    pub fn take(&self) -> Option<T> {
        self.0.lock().unwrap().take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A status port on `127.0.0.1` answering each request, given with its index, by `reply`.
    fn fake_server(
        reply: impl Fn(StatusRequest, usize) -> Option<StatusReply> + Send + 'static,
    ) -> SocketAddr {
        let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
        let address = socket.local_addr().unwrap();
        // Stops once no request has come for a while, after its test
        socket.set_read_timeout(Some(STATUS_TIMEOUT * 2)).unwrap();
        thread::spawn(move || {
            let mut buffer = [0; STATUS_PACKET_LEN];
            let mut index = 0;
            while let Ok((len, from)) = socket.recv_from(&mut buffer) {
                let Some(request) = StatusRequest::decode(&buffer[..len]) else {
                    continue;
                };
                if let Some(answer) = reply(request, index) {
                    socket.send_to(&answer.encode(), from).unwrap();
                }
                index += 1;
            }
        });
        address
    }

    fn reply(request: StatusRequest, protocol: u64) -> StatusReply {
        StatusReply {
            nonce: request.nonce,
            protocol,
            players: 3,
            max_players: 20,
        }
    }

    #[test]
    fn a_server_that_answers_is_online_with_its_players_and_latency() {
        let server = fake_server(|request, _| Some(reply(request, PROTOCOL_ID)));

        let status = query_status_at(server, STATUS_TIMEOUT);

        let ServerStatus::Online {
            players,
            max_players,
            latency,
        } = status
        else {
            panic!("{status:?}");
        };
        assert_eq!((players, max_players), (3, 20));
        assert!(latency < Duration::from_secs(1), "{latency:?}");
    }

    #[test]
    fn a_server_of_another_protocol_is_incompatible() {
        let server = fake_server(|request, _| Some(reply(request, PROTOCOL_ID + 1)));

        assert_eq!(
            query_status_at(server, STATUS_TIMEOUT),
            ServerStatus::Incompatible
        );
    }

    #[test]
    fn a_lost_request_is_sent_again() {
        let server =
            fake_server(|request, index| (index == 1).then(|| reply(request, PROTOCOL_ID)));

        let status = query_status_at(server, STATUS_TIMEOUT);

        let ServerStatus::Online { latency, .. } = status else {
            panic!("{status:?}");
        };
        assert!(latency < RESEND_EVERY, "{latency:?}");
    }

    #[test]
    fn a_reply_with_another_nonce_is_ignored() {
        let server = fake_server(|request, _| {
            Some(reply(
                StatusRequest {
                    nonce: request.nonce.wrapping_add(1),
                },
                PROTOCOL_ID,
            ))
        });
        let start = Instant::now();

        assert_eq!(
            query_status_at(server, STATUS_TIMEOUT),
            ServerStatus::Offline
        );
        assert!(start.elapsed() >= STATUS_TIMEOUT, "{:?}", start.elapsed());
    }

    #[test]
    fn nothing_listening_is_offline_soon() {
        let closed = UdpSocket::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap();
        let start = Instant::now();

        assert_eq!(
            query_status_at(closed, STATUS_TIMEOUT),
            ServerStatus::Offline
        );
        // A refused port is told at once, before the first resend
        assert!(start.elapsed() < RESEND_EVERY, "{:?}", start.elapsed());
    }

    #[test]
    fn an_unknown_host_or_a_bad_address_is_said_so() {
        assert_eq!(query_status("nowhere.invalid"), ServerStatus::UnknownHost);
        assert_eq!(query_status("bad host"), ServerStatus::BadAddress);
    }

    #[test]
    fn latency_reads_as_bars() {
        for (ms, bars) in [(10, 4), (60, 4), (61, 3), (120, 3), (200, 2), (251, 1)] {
            assert_eq!(latency_bars(Duration::from_millis(ms)), bars, "{ms} ms");
        }
    }

    #[test]
    fn in_flight_gives_its_result_once() {
        let in_flight = InFlight::spawn(|| 5);
        let start = Instant::now();

        let result = loop {
            if let Some(result) = in_flight.take() {
                break result;
            }
            assert!(start.elapsed() < Duration::from_secs(5), "never finished");
            thread::yield_now();
        };

        assert_eq!(result, 5);
        assert_eq!(in_flight.take(), None);
    }
}
