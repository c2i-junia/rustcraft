//! The challenge a server gives each connection, and the check of the proof a player of a hub
//! answers it with: its certificate, and the signature of its device over the challenge.

use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::Path;

use bevy::prelude::*;
use bevy_renet::renet::ClientId;
use bevy_renet::RenetServer;
use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;
use rand::RngCore;
use rustcraft_protocol::identity::{
    key_from_hex, key_to_hex, player_id, verify_join, Certificate, CertificateError,
    SignedCertificate,
};
use serde::{Deserialize, Serialize};
use shared::messages::{JoinProof, ServerToClientMessage};
use shared::{GameServerConfig, HubTrust, ServerAuth};

use super::dispatcher::ServerEventMessage;
use super::extensions::SendGameMessageExtension;

/// The key of the server, in its game folder: the players of a hub sign it with the challenge,
/// so that a signature made for one server is worth nothing on another.
pub const SERVER_KEY_FILE: &str = "server_key.ron";

/// The reason given to a player without a proof by a server in hub mode.
pub const NO_PROOF: &str = "This server needs a Rustcraft account: log in from the main menu.";
pub const EXPIRED: &str = "The certificate of your account has expired: the game renews it as soon as it reaches the hub.";
pub const NOT_YET_VALID: &str =
    "The certificate of your account is not valid yet: check the clock of your computer.";
pub const UNKNOWN_HUB: &str = "Your account is not one of the hub of this server.";
pub const MALFORMED: &str = "The certificate of your account could not be read.";
pub const REVOKED: &str =
    "The certificate of your account was revoked: log in again from the main menu.";
pub const WRONG_SIGNATURE: &str = "The proof of your account was not made for this connection.";
pub const WRONG_ID: &str = "Your connection does not match your account.";

/// The public half of the key of the server.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ServerKey(pub [u8; 32]);

/// A key of its own, for the servers of the tests: a dedicated server reads its file instead
/// (`init::run`).
impl Default for ServerKey {
    fn default() -> Self {
        Self(new_key().verifying_key().to_bytes())
    }
}

#[derive(Serialize, Deserialize)]
struct ServerKeyFile {
    /// The seed of the Ed25519 key, hex.
    seed: String,
}

fn new_key() -> SigningKey {
    let mut seed = [0u8; 32];
    OsRng.fill_bytes(&mut seed);
    SigningKey::from_bytes(&seed)
}

impl ServerKey {
    /// The key of `SERVER_KEY_FILE` in `folder`, made and written there when there is none. A
    /// file that cannot be read is an error: the players would see another server.
    pub fn load_or_create(folder: &Path) -> Result<Self, String> {
        let path = folder.join(SERVER_KEY_FILE);
        match fs::read_to_string(&path) {
            Ok(text) => {
                let file: ServerKeyFile = ron::from_str(&text)
                    .map_err(|err| format!("Could not read {}: {err}", path.display()))?;
                let seed = key_from_hex(&file.seed)
                    .ok_or_else(|| format!("{} does not hold 32 bytes of hex", path.display()))?;
                Ok(Self(
                    SigningKey::from_bytes(&seed).verifying_key().to_bytes(),
                ))
            }
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                let key = new_key();
                let file = ServerKeyFile {
                    seed: key_to_hex(&key.to_bytes()),
                };
                let text = ron::ser::to_string_pretty(&file, Default::default())
                    .map_err(|err| err.to_string())?;
                write_private(folder, &path, text.as_bytes())
                    .map_err(|err| format!("Could not write {}: {err}", path.display()))?;
                info!("Made the key of this server in {}", path.display());
                Ok(Self(key.verifying_key().to_bytes()))
            }
            Err(err) => Err(format!("Could not read {}: {err}", path.display())),
        }
    }
}

/// Writes a file readable by its owner only on Unix, through a temporary file renamed over it.
fn write_private(folder: &Path, path: &Path, data: &[u8]) -> io::Result<()> {
    fs::create_dir_all(folder)?;
    let tmp = path.with_extension("ron.tmp");
    let _ = fs::remove_file(&tmp);
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&tmp)?;
    file.write_all(data)?;
    file.sync_all()?;
    fs::rename(&tmp, path)
}

/// The nonce given to each connected client, forgotten when it disconnects.
#[derive(Resource, Default, Debug)]
pub struct Challenges(HashMap<ClientId, [u8; 32]>);

impl Challenges {
    pub fn get(&self, client_id: ClientId) -> Option<&[u8; 32]> {
        self.0.get(&client_id)
    }
}

/// Gives each new connection a nonce of its own, at once, with the key of the server and the
/// hub whose accounts it takes: the client answers with its request to join.
pub(super) fn send_challenges_system(
    mut events: MessageReader<ServerEventMessage>,
    mut server: ResMut<RenetServer>,
    mut challenges: ResMut<Challenges>,
    key: Res<ServerKey>,
    config: Res<GameServerConfig>,
) {
    for event in events.read() {
        match event {
            ServerEventMessage::ClientConnected { client_id } => {
                let mut nonce = [0u8; 32];
                OsRng.fill_bytes(&mut nonce);
                challenges.0.insert(*client_id, nonce);
                let hub = match &config.auth {
                    ServerAuth::Offline => None,
                    ServerAuth::Hub(trust) => Some(trust.url.clone()),
                };
                server.send_game_message(
                    *client_id,
                    ServerToClientMessage::AuthChallenge {
                        nonce,
                        server_key: key.0,
                        hub,
                    },
                );
            }
            ServerEventMessage::ClientDisconnected { client_id, .. } => {
                challenges.0.remove(client_id);
            }
        }
    }
}

/// The certificate of the proof of `client_id`, when it is one of `trust`'s hub, not revoked,
/// its device signed the nonce of this connection for this server, and the account is the one
/// of the client id. Otherwise the reason to give the player.
pub fn check_proof(
    trust: &HubTrust,
    proof: Option<&JoinProof>,
    client_id: ClientId,
    nonce: Option<&[u8; 32]>,
    server_key: &[u8; 32],
    now: u64,
) -> Result<Certificate, &'static str> {
    let proof = proof.ok_or(NO_PROOF)?;
    let signed = SignedCertificate {
        bytes: proof.certificate.clone(),
        signature: proof
            .certificate_signature
            .as_slice()
            .try_into()
            .map_err(|_| MALFORMED)?,
    };
    let certificate = signed.verify(&trust.keys, now).map_err(|err| match err {
        CertificateError::UnknownKey | CertificateError::BadSignature => UNKNOWN_HUB,
        CertificateError::Malformed => MALFORMED,
        CertificateError::NotYetValid => NOT_YET_VALID,
        CertificateError::Expired => EXPIRED,
    })?;
    let revoked = trust
        .revoked
        .read()
        .is_ok_and(|revoked| revoked.contains(&certificate.id));
    if revoked {
        return Err(REVOKED);
    }
    let signature: [u8; 64] = proof
        .signature
        .as_slice()
        .try_into()
        .map_err(|_| WRONG_SIGNATURE)?;
    let nonce = nonce.ok_or(WRONG_SIGNATURE)?;
    if !verify_join(&certificate, nonce, server_key, &signature) {
        return Err(WRONG_SIGNATURE);
    }
    // The client announces its own id: one that is not the account's could take its place
    if player_id(&certificate.account) != client_id {
        return Err(WRONG_ID);
    }
    Ok(certificate)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustcraft_protocol::identity::{
        sign, sign_join, CERTIFICATE_LIFETIME_SECS, CERTIFICATE_VERSION,
    };
    use std::collections::HashSet;
    use std::sync::{Arc, RwLock};
    use uuid::Uuid;

    const NOW: u64 = 2_000_000_000;
    const NONCE: [u8; 32] = [3; 32];
    const SERVER: [u8; 32] = [4; 32];

    struct Fixture {
        hub: SigningKey,
        device: SigningKey,
        certificate: Certificate,
    }

    impl Fixture {
        fn new() -> Self {
            let device = new_key();
            Self {
                hub: new_key(),
                certificate: Certificate {
                    version: CERTIFICATE_VERSION,
                    hub: "https://hub.example".into(),
                    id: Uuid::from_u128(7),
                    account: Uuid::from_u128(0x0102030405060708_090a0b0c0d0e0f10),
                    name: "Alex".into(),
                    device_key: device.verifying_key().to_bytes(),
                    issued_at: NOW - 10,
                    expires_at: NOW + CERTIFICATE_LIFETIME_SECS,
                    hub_key_id: 1,
                },
                device,
            }
        }

        fn trust(&self) -> HubTrust {
            HubTrust {
                url: "https://hub.example".into(),
                keys: vec![(1, self.hub.verifying_key())],
                revoked: Arc::new(RwLock::new(HashSet::new())),
            }
        }

        fn proof(&self, nonce: &[u8; 32], server: &[u8; 32]) -> JoinProof {
            let signed = sign(&self.certificate, &self.hub);
            JoinProof {
                certificate: signed.bytes,
                certificate_signature: signed.signature.to_vec(),
                signature: sign_join(&self.device, nonce, server).to_vec(),
            }
        }

        fn id(&self) -> ClientId {
            player_id(&self.certificate.account)
        }

        fn check(&self, trust: &HubTrust, proof: &JoinProof) -> Result<Certificate, &str> {
            check_proof(trust, Some(proof), self.id(), Some(&NONCE), &SERVER, NOW)
        }
    }

    #[test]
    fn a_proof_for_this_connection_gives_its_certificate() {
        let fixture = Fixture::new();
        let proof = fixture.proof(&NONCE, &SERVER);
        assert_eq!(
            fixture.check(&fixture.trust(), &proof),
            Ok(fixture.certificate.clone())
        );
    }

    #[test]
    fn each_failure_has_its_own_reason() {
        let fixture = Fixture::new();
        let trust = fixture.trust();
        let proof = fixture.proof(&NONCE, &SERVER);
        assert_eq!(
            check_proof(&trust, None, fixture.id(), Some(&NONCE), &SERVER, NOW),
            Err(NO_PROOF)
        );
        assert_eq!(
            check_proof(&trust, Some(&proof), fixture.id(), None, &SERVER, NOW),
            Err(WRONG_SIGNATURE)
        );
        assert_eq!(
            check_proof(
                &trust,
                Some(&proof),
                fixture.id() + 1,
                Some(&NONCE),
                &SERVER,
                NOW
            ),
            Err(WRONG_ID)
        );
        let late = NOW + CERTIFICATE_LIFETIME_SECS;
        assert_eq!(
            check_proof(
                &trust,
                Some(&proof),
                fixture.id(),
                Some(&NONCE),
                &SERVER,
                late
            ),
            Err(EXPIRED)
        );
        assert_eq!(
            fixture.check(&trust, &fixture.proof(&[9; 32], &SERVER)),
            Err(WRONG_SIGNATURE)
        );
        assert_eq!(
            fixture.check(&trust, &fixture.proof(&NONCE, &[9; 32])),
            Err(WRONG_SIGNATURE)
        );
        let other_hub = Fixture::new().trust();
        assert_eq!(fixture.check(&other_hub, &proof), Err(UNKNOWN_HUB));
        let mut short = proof.clone();
        short.certificate_signature.pop();
        assert_eq!(fixture.check(&trust, &short), Err(MALFORMED));
        trust
            .revoked
            .write()
            .unwrap()
            .insert(fixture.certificate.id);
        assert_eq!(fixture.check(&trust, &proof), Err(REVOKED));
    }

    #[test]
    fn the_key_of_the_server_is_made_once_and_kept() {
        let dir = tempfile::tempdir().unwrap();
        let key = ServerKey::load_or_create(dir.path()).unwrap();
        assert_eq!(ServerKey::load_or_create(dir.path()).unwrap(), key);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(dir.path().join(SERVER_KEY_FILE))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        fs::write(dir.path().join(SERVER_KEY_FILE), "(seed: \"abcd\")").unwrap();
        assert!(ServerKey::load_or_create(dir.path()).is_err());
    }
}
