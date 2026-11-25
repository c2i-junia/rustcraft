//! Device certificates signed by the hub, and the signature a device gives a server to join.

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The only certificate layout there is for now.
pub const CERTIFICATE_VERSION: u8 = 1;

/// How far in the future a certificate may be dated, in seconds, to forgive clock drift.
pub const CLOCK_SKEW_SECS: u64 = 300;

/// How long a certificate lives, in seconds: 7 days.
pub const CERTIFICATE_LIFETIME_SECS: u64 = 7 * 24 * 3600;

const JOIN_PREFIX: &[u8] = b"rustcraft-join-v1";

/// What the hub vouches for: this device key belongs to this account.
///
/// The fields are encoded with `postcard` in this order, and `hub_key_id` stays the last one:
/// [`SignedCertificate::verify`] reads it from the final byte to pick the key before decoding.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Certificate {
    pub version: u8,
    /// The public URL of the hub that signed it.
    pub hub: String,
    /// The id of this certificate, the one `GET /v1/revoked` lists.
    pub id: Uuid,
    pub account: Uuid,
    /// The display name of the account.
    pub name: String,
    pub device_key: [u8; 32],
    /// Unix seconds.
    pub issued_at: u64,
    /// Unix seconds.
    pub expires_at: u64,
    pub hub_key_id: u8,
}

/// The exact bytes of a [`Certificate`] and the hub's signature over them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignedCertificate {
    pub bytes: Vec<u8>,
    pub signature: [u8; 64],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CertificateError {
    /// The key id is not one of the accepted keys.
    UnknownKey,
    BadSignature,
    /// The signed bytes are not a certificate of a known layout.
    Malformed,
    /// Issued more than [`CLOCK_SKEW_SECS`] after `now`.
    NotYetValid,
    Expired,
}

impl std::fmt::Display for CertificateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::UnknownKey => "the certificate is signed by an unknown hub key",
            Self::BadSignature => "the certificate signature is wrong",
            Self::Malformed => "the certificate is malformed",
            Self::NotYetValid => "the certificate is not valid yet",
            Self::Expired => "the certificate has expired",
        })
    }
}

impl std::error::Error for CertificateError {}

/// Signs a certificate with the hub key whose id it names.
pub fn sign(cert: &Certificate, key: &SigningKey) -> SignedCertificate {
    let bytes = postcard::to_allocvec(cert).expect("a certificate always encodes");
    let signature = key.sign(&bytes).to_bytes();
    SignedCertificate { bytes, signature }
}

impl SignedCertificate {
    /// Checks the key id, the signature, then decodes the bytes and checks the dates against
    /// `now` (Unix seconds). `keys` are the accepted hub keys with their ids.
    ///
    /// It does not check `cert.hub`: callers must pass only the keys of the hub they trust.
    pub fn verify(
        &self,
        keys: &[(u8, VerifyingKey)],
        now: u64,
    ) -> Result<Certificate, CertificateError> {
        let key_id = *self.bytes.last().ok_or(CertificateError::Malformed)?;
        let (_, key) = keys
            .iter()
            .find(|(id, _)| *id == key_id)
            .ok_or(CertificateError::UnknownKey)?;
        key.verify_strict(&self.bytes, &Signature::from_bytes(&self.signature))
            .map_err(|_| CertificateError::BadSignature)?;
        let (cert, rest) = postcard::take_from_bytes::<Certificate>(&self.bytes)
            .map_err(|_| CertificateError::Malformed)?;
        if !rest.is_empty() {
            return Err(CertificateError::Malformed);
        }
        if cert.version != CERTIFICATE_VERSION || cert.hub_key_id != key_id {
            return Err(CertificateError::Malformed);
        }
        if cert.issued_at > now.saturating_add(CLOCK_SKEW_SECS) {
            return Err(CertificateError::NotYetValid);
        }
        if now >= cert.expires_at {
            return Err(CertificateError::Expired);
        }
        Ok(cert)
    }
}

/// What a device signs to join a server: the server's challenge bound to the server's key and
/// to the address the player dialed (`crate::normalize_address`), so that a server cannot pass
/// the challenge of another one to its players and join there as them.
pub fn join_message(nonce: &[u8; 32], server_key: &[u8; 32], address: &str) -> Vec<u8> {
    let mut message = Vec::with_capacity(JOIN_PREFIX.len() + 64 + address.len());
    message.extend_from_slice(JOIN_PREFIX);
    message.extend_from_slice(nonce);
    message.extend_from_slice(server_key);
    message.extend_from_slice(address.as_bytes());
    message
}

/// The join signature of a device.
pub fn sign_join(
    device: &SigningKey,
    nonce: &[u8; 32],
    server_key: &[u8; 32],
    address: &str,
) -> [u8; 64] {
    device
        .sign(&join_message(nonce, server_key, address))
        .to_bytes()
}

/// Checks that the device key of `cert` signed this nonce for this server at this address.
pub fn verify_join(
    cert: &Certificate,
    nonce: &[u8; 32],
    server_key: &[u8; 32],
    address: &str,
    signature: &[u8; 64],
) -> bool {
    let Ok(key) = VerifyingKey::from_bytes(&cert.device_key) else {
        return false;
    };
    key.verify_strict(
        &join_message(nonce, server_key, address),
        &Signature::from_bytes(signature),
    )
    .is_ok()
}

/// The id of the player of an account in the game: the first 8 bytes of its UUID,
/// little-endian. The game and the servers that check certificates derive the same one.
pub fn player_id(account: &Uuid) -> u64 {
    let mut first = [0u8; 8];
    first.copy_from_slice(&account.as_bytes()[..8]);
    u64::from_le_bytes(first)
}

/// The hex form of a 32-byte key, as the JSON of the hub carries it.
pub fn key_to_hex(key: &[u8; 32]) -> String {
    hex::encode(key)
}

/// Reads a 32-byte key from hex, `None` when it has another length or other characters.
pub fn key_from_hex(text: &str) -> Option<[u8; 32]> {
    hex::decode(text).ok()?.try_into().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::OsRng;

    const NOW: u64 = 1_000_000;

    fn key() -> SigningKey {
        SigningKey::generate(&mut OsRng)
    }

    fn certificate(device: &SigningKey) -> Certificate {
        Certificate {
            version: CERTIFICATE_VERSION,
            hub: "https://hub.example".into(),
            id: Uuid::new_v4(),
            account: Uuid::new_v4(),
            name: "Alex".into(),
            device_key: device.verifying_key().to_bytes(),
            issued_at: NOW - 10,
            expires_at: NOW + CERTIFICATE_LIFETIME_SECS,
            hub_key_id: 1,
        }
    }

    fn keys(hub: &SigningKey) -> Vec<(u8, VerifyingKey)> {
        vec![(1, hub.verifying_key())]
    }

    #[test]
    fn a_signed_certificate_verifies() {
        let (hub, device) = (key(), key());
        let cert = certificate(&device);
        let signed = sign(&cert, &hub);
        assert_eq!(signed.verify(&keys(&hub), NOW), Ok(cert));
    }

    #[test]
    fn a_flipped_byte_is_refused() {
        let (hub, device) = (key(), key());
        let mut signed = sign(&certificate(&device), &hub);
        signed.bytes[0] ^= 1;
        assert_eq!(
            signed.verify(&keys(&hub), NOW),
            Err(CertificateError::BadSignature)
        );
        let mut signed = sign(&certificate(&device), &hub);
        signed.signature[0] ^= 1;
        assert_eq!(
            signed.verify(&keys(&hub), NOW),
            Err(CertificateError::BadSignature)
        );
    }

    #[test]
    fn a_certificate_signed_by_another_key_is_refused() {
        let (hub, device) = (key(), key());
        let signed = sign(&certificate(&device), &key());
        assert_eq!(
            signed.verify(&keys(&hub), NOW),
            Err(CertificateError::BadSignature)
        );
    }

    #[test]
    fn an_unknown_key_id_is_refused() {
        let (hub, device) = (key(), key());
        let mut cert = certificate(&device);
        cert.hub_key_id = 2;
        let signed = sign(&cert, &hub);
        assert_eq!(
            signed.verify(&keys(&hub), NOW),
            Err(CertificateError::UnknownKey)
        );
    }

    #[test]
    fn an_expired_certificate_is_refused() {
        let (hub, device) = (key(), key());
        let cert = certificate(&device);
        let signed = sign(&cert, &hub);
        assert_eq!(
            signed.verify(&keys(&hub), cert.expires_at),
            Err(CertificateError::Expired)
        );
        assert!(signed.verify(&keys(&hub), cert.expires_at - 1).is_ok());
    }

    #[test]
    fn a_certificate_from_the_future_is_refused_beyond_the_clock_skew() {
        let (hub, device) = (key(), key());
        let mut cert = certificate(&device);
        cert.issued_at = NOW + CLOCK_SKEW_SECS;
        let signed = sign(&cert, &hub);
        assert!(signed.verify(&keys(&hub), NOW).is_ok());
        cert.issued_at += 1;
        let signed = sign(&cert, &hub);
        assert_eq!(
            signed.verify(&keys(&hub), NOW),
            Err(CertificateError::NotYetValid)
        );
    }

    #[test]
    fn an_empty_certificate_is_malformed() {
        let hub = key();
        let signed = SignedCertificate {
            bytes: Vec::new(),
            signature: [0; 64],
        };
        assert_eq!(
            signed.verify(&keys(&hub), NOW),
            Err(CertificateError::Malformed)
        );
    }

    #[test]
    fn signed_trailing_bytes_or_garbage_are_malformed() {
        let (hub, device) = (key(), key());
        let mut bytes = postcard::to_allocvec(&certificate(&device)).unwrap();
        // The extra byte is the key id, so only the trailing data is wrong.
        bytes.push(1);
        let signature = hub.sign(&bytes).to_bytes();
        let signed = SignedCertificate { bytes, signature };
        assert_eq!(
            signed.verify(&keys(&hub), NOW),
            Err(CertificateError::Malformed)
        );
        let bytes = vec![1u8, 2, 3, 1];
        let signature = hub.sign(&bytes).to_bytes();
        let signed = SignedCertificate { bytes, signature };
        assert_eq!(
            signed.verify(&keys(&hub), NOW),
            Err(CertificateError::Malformed)
        );
    }

    #[test]
    fn the_key_id_is_the_last_byte_of_the_encoded_certificate() {
        let cert = certificate(&key());
        let bytes = postcard::to_allocvec(&cert).unwrap();
        assert_eq!(bytes.last(), Some(&cert.hub_key_id));
    }

    #[test]
    fn a_join_signature_verifies_only_for_its_nonce_server_key_and_address() {
        let device = key();
        let cert = certificate(&device);
        let (nonce, server, address) = ([7u8; 32], [9u8; 32], "play.example:8000");
        let signature = sign_join(&device, &nonce, &server, address);
        assert!(verify_join(&cert, &nonce, &server, address, &signature));
        assert!(!verify_join(
            &cert, &[8u8; 32], &server, address, &signature
        ));
        assert!(!verify_join(&cert, &nonce, &[1u8; 32], address, &signature));
        assert!(!verify_join(
            &cert,
            &nonce,
            &server,
            "evil.example:8000",
            &signature
        ));
        let other = certificate(&key());
        assert!(!verify_join(&other, &nonce, &server, address, &signature));
    }

    #[test]
    fn the_player_id_is_the_first_eight_bytes_of_the_uuid_little_endian() {
        let uuid = Uuid::from_bytes([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);
        assert_eq!(player_id(&uuid), 0x0807060504030201);
    }

    #[test]
    fn keys_round_trip_through_hex() {
        let key = [0xAB; 32];
        let text = key_to_hex(&key);
        assert_eq!(text.len(), 64);
        assert_eq!(key_from_hex(&text), Some(key));
        assert_eq!(key_from_hex("abcd"), None);
        assert_eq!(key_from_hex(&"zz".repeat(32)), None);
    }
}
