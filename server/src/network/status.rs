//! Answers status requests (see `shared::status`) on the port next to the game port. The game
//! socket belongs to netcode, which drops whatever is not netcode, so the status has its own.
//! It is answered from its own thread: the latency a client measures is the network's, not the
//! tick's, and nothing a client sends there can slow the game loop.

use crate::init::ServerLobby;
use bevy::prelude::*;
use shared::status::{StatusReply, StatusRequest, STATUS_PACKET_LEN};
use shared::PROTOCOL_ID;
use std::io::ErrorKind;
use std::net::UdpSocket;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Weak};
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// How long the responder waits for a request before checking that the server still runs.
const RECEIVE_TIMEOUT: Duration = Duration::from_millis(250);

/// How long the responder pauses after an unexpected receive error, so that a lasting one
/// does not spin it.
const ERROR_PAUSE: Duration = Duration::from_millis(10);

/// The Windows error (`WSAEMSGSIZE`) of a datagram longer than the receive buffer, which
/// Windows drops instead of truncating. No other platform has an error of this number.
const WSAEMSGSIZE: i32 = 10040;

/// The number of players the responder reports. The app owns the only strong reference: the
/// responder stops once it is dropped.
#[derive(Resource)]
pub(crate) struct StatusCounter(pub Arc<AtomicU32>);

/// The reply to `packet`, or nothing when it is not a well-formed status request.
pub(crate) fn answer(
    packet: &[u8],
    players: u32,
    max_players: u32,
) -> Option<[u8; STATUS_PACKET_LEN]> {
    let request = StatusRequest::decode(packet)?;
    Some(
        StatusReply {
            nonce: request.nonce,
            protocol: PROTOCOL_ID,
            players,
            max_players,
        }
        .encode(),
    )
}

/// Answers on `socket` until `players` can no longer be upgraded (the server app is gone).
pub(crate) fn spawn_status_responder(
    socket: UdpSocket,
    players: Weak<AtomicU32>,
    max_players: u32,
) -> std::io::Result<JoinHandle<()>> {
    socket.set_read_timeout(Some(RECEIVE_TIMEOUT))?;
    thread::Builder::new()
        .name("status responder".into())
        .spawn(move || {
            // One byte more than a request, so that a longer datagram is not cut down to one
            let mut buffer = [0; STATUS_PACKET_LEN + 1];
            loop {
                let received = socket.recv_from(&mut buffer);
                let Some(players) = players.upgrade() else {
                    return;
                };
                match received {
                    Ok((len, from)) => {
                        let count = players.load(Ordering::Relaxed);
                        if let Some(reply) = answer(&buffer[..len], count, max_players) {
                            if let Err(err) = socket.send_to(&reply, from) {
                                debug!("Could not answer the status request of {from}: {err}");
                            }
                        }
                    }
                    Err(err)
                        if matches!(err.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
                    // How Windows reports the ICMP refusal of an earlier reply: nothing is wrong here
                    Err(err) if err.kind() == ErrorKind::ConnectionReset => {}
                    // An oversized datagram on Windows: one malformed request, dropped
                    Err(err) if err.raw_os_error() == Some(WSAEMSGSIZE) => {}
                    Err(err) => {
                        debug!("Could not receive a status request: {err}");
                        thread::sleep(ERROR_PAUSE);
                    }
                }
            }
        })
}

/// Hands the number of players to the responder.
pub(crate) fn publish_player_count(lobby: Res<ServerLobby>, counter: Res<StatusCounter>) {
    counter
        .0
        .store(lobby.players.len() as u32, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn only_a_good_request_is_answered() {
        let reply = answer(&StatusRequest { nonce: 9 }.encode(), 2, 64).unwrap();
        assert_eq!(
            StatusReply::decode(&reply),
            Some(StatusReply {
                nonce: 9,
                protocol: PROTOCOL_ID,
                players: 2,
                max_players: 64,
            })
        );

        let request = StatusRequest { nonce: 9 }.encode();
        let mut longer = request.to_vec();
        longer.push(0);
        let mut wrong_magic = request;
        wrong_magic[0] = b'X';
        let mut format_2 = request;
        format_2[4] = 2;
        for packet in [
            &[][..],
            &[0],
            &request[..31],
            &longer,
            &wrong_magic,
            &format_2,
            &reply,
        ] {
            assert_eq!(answer(packet, 2, 64), None, "{packet:?}");
        }
    }

    #[test]
    fn the_responder_answers_until_the_server_is_gone() {
        let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
        let addr = socket.local_addr().unwrap();
        let players = Arc::new(AtomicU32::new(3));
        let responder = spawn_status_responder(socket, Arc::downgrade(&players), 64).unwrap();

        let client = UdpSocket::bind("127.0.0.1:0").unwrap();
        client.connect(addr).unwrap();
        client
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        client.send(&StatusRequest { nonce: 1 }.encode()).unwrap();
        let mut buffer = [0; 64];
        let len = client.recv(&mut buffer).unwrap();
        assert_eq!(
            StatusReply::decode(&buffer[..len]).map(|reply| reply.players),
            Some(3)
        );

        drop(players);
        let start = Instant::now();
        while !responder.is_finished() {
            assert!(
                start.elapsed() < Duration::from_secs(2),
                "the responder outlived the server"
            );
            thread::sleep(Duration::from_millis(10));
        }
        responder.join().unwrap();
    }
}
