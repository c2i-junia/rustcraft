//! The status exchange: a client asks a server how many players it has and which protocol it
//! speaks, before joining it. It lives outside netcode, on its own UDP port next to the game
//! port, and is versioned by its own magic and format instead of `PROTOCOL_ID`.
//!
//! A packet is 32 bytes, little-endian, zero padded: magic (4), format (1), kind (1: `0`
//! request, `1` reply), nonce (8), and for a reply protocol (8), players (4), max players (4).
//! A request is as large as a reply, so a spoofed request cannot amplify traffic. Format 1
//! never changes: a new layout gets a new format.

pub const STATUS_MAGIC: [u8; 4] = *b"RCST";
pub const STATUS_FORMAT: u8 = 1;
/// The size of every status packet, request or reply.
pub const STATUS_PACKET_LEN: usize = 32;

const KIND_REQUEST: u8 = 0;
const KIND_REPLY: u8 = 1;

/// Asks a server for its status; the reply carries the same nonce.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StatusRequest {
    pub nonce: u64,
}

/// What a server says of itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StatusReply {
    /// The nonce of the request this answers.
    pub nonce: u64,
    /// The server's `PROTOCOL_ID`: a client of another one cannot join it.
    pub protocol: u64,
    pub players: u32,
    pub max_players: u32,
}

impl StatusRequest {
    pub fn encode(&self) -> [u8; STATUS_PACKET_LEN] {
        header(KIND_REQUEST, self.nonce)
    }

    /// The request in `packet`, or nothing when it is not a well-formed format 1 request.
    pub fn decode(packet: &[u8]) -> Option<Self> {
        let packet = checked(packet, KIND_REQUEST)?;
        Some(Self {
            nonce: u64::from_le_bytes(field(packet, 6)),
        })
    }
}

impl StatusReply {
    pub fn encode(&self) -> [u8; STATUS_PACKET_LEN] {
        let mut packet = header(KIND_REPLY, self.nonce);
        packet[14..22].copy_from_slice(&self.protocol.to_le_bytes());
        packet[22..26].copy_from_slice(&self.players.to_le_bytes());
        packet[26..30].copy_from_slice(&self.max_players.to_le_bytes());
        packet
    }

    /// The reply in `packet`, or nothing when it is not a well-formed format 1 reply.
    pub fn decode(packet: &[u8]) -> Option<Self> {
        let packet = checked(packet, KIND_REPLY)?;
        Some(Self {
            nonce: u64::from_le_bytes(field(packet, 6)),
            protocol: u64::from_le_bytes(field(packet, 14)),
            players: u32::from_le_bytes(field(packet, 22)),
            max_players: u32::from_le_bytes(field(packet, 26)),
        })
    }
}

/// The UDP port of the status of a server listening on `game_port`.
pub fn status_port(game_port: u16) -> Option<u16> {
    game_port.checked_add(1)
}

/// A zero padded packet of `kind` up to its nonce.
fn header(kind: u8, nonce: u64) -> [u8; STATUS_PACKET_LEN] {
    let mut packet = [0; STATUS_PACKET_LEN];
    packet[0..4].copy_from_slice(&STATUS_MAGIC);
    packet[4] = STATUS_FORMAT;
    packet[5] = kind;
    packet[6..14].copy_from_slice(&nonce.to_le_bytes());
    packet
}

/// `packet`, when it is exactly one format 1 packet of `kind`.
fn checked(packet: &[u8], kind: u8) -> Option<&[u8; STATUS_PACKET_LEN]> {
    let packet: &[u8; STATUS_PACKET_LEN] = packet.try_into().ok()?;
    (packet[0..4] == STATUS_MAGIC && packet[4] == STATUS_FORMAT && packet[5] == kind)
        .then_some(packet)
}

/// The `N` bytes of `packet` from `start`.
fn field<const N: usize>(packet: &[u8; STATUS_PACKET_LEN], start: usize) -> [u8; N] {
    std::array::from_fn(|i| packet[start + i])
}

#[cfg(test)]
mod tests {
    use super::*;

    const REPLY: StatusReply = StatusReply {
        nonce: 7,
        protocol: 5,
        players: 3,
        max_players: 64,
    };

    #[test]
    fn a_request_and_a_reply_read_back_what_was_written() {
        let request = StatusRequest { nonce: 7 }.encode();
        let reply = REPLY.encode();

        assert_eq!(request.len(), 32);
        assert_eq!(reply.len(), 32);
        assert_eq!(
            StatusRequest::decode(&request),
            Some(StatusRequest { nonce: 7 })
        );
        assert_eq!(StatusReply::decode(&reply), Some(REPLY));
    }

    #[test]
    fn malformed_packets_decode_to_nothing() {
        let request = StatusRequest { nonce: 7 }.encode();
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
            &REPLY.encode(),
        ] {
            assert_eq!(StatusRequest::decode(packet), None, "{packet:?}");
        }
        let mut reply_format_2 = REPLY.encode();
        reply_format_2[4] = 2;
        for packet in [&request[..], &reply_format_2] {
            assert_eq!(StatusReply::decode(packet), None, "{packet:?}");
        }
    }

    #[test]
    fn the_status_port_is_next_to_the_game_port() {
        assert_eq!(status_port(8000), Some(8001));
        assert_eq!(status_port(65535), None);
    }
}
