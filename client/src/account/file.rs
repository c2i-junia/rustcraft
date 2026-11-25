//! `account.ron` in the game folder: the account the player is signed in to, and what the game
//! needs from the hub to join servers with it.

use std::fs;
use std::io;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;
use rand::RngCore;
use ron::ser::PrettyConfig;
use rustcraft_protocol::api::{AccountSummary, CertificateResponse};
use rustcraft_protocol::identity::{key_from_hex, key_to_hex, CERTIFICATE_LIFETIME_SECS};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::atomic_write::write_atomically;

/// The account of the player on a hub. It holds secrets (the session token and the seed of the
/// device key), so it is written readable by its owner only.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct AccountFile {
    /// The base URL of the hub the account is on.
    pub hub: String,
    pub username: String,
    pub account: Uuid,
    /// The bearer token of the session.
    pub session: String,
    /// The seed of the Ed25519 device key, hex. Made once with the file: it never leaves this
    /// computer, only its public half goes to the hub.
    pub device_key: String,
    /// The last certificate and its signature, base64, as the hub sent them.
    #[serde(default)]
    pub certificate: Option<String>,
    #[serde(default)]
    pub signature: Option<String>,
    /// When the certificate expires, in Unix seconds.
    #[serde(default)]
    pub expires_at: Option<u64>,
}

/// Prints everything but the secrets: a `{:?}` of the account must not leak the session or the
/// device key into a log or a failed assertion.
impl std::fmt::Debug for AccountFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AccountFile")
            .field("hub", &self.hub)
            .field("username", &self.username)
            .field("account", &self.account)
            .field("session", &"<redacted>")
            .field("device_key", &"<redacted>")
            .field("certificate", &self.certificate)
            .field("signature", &self.signature)
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

impl AccountFile {
    /// A signed-in account with a new device key and no certificate yet.
    pub fn new(hub: &str, account: &AccountSummary, session: String) -> Self {
        let mut seed = [0u8; 32];
        OsRng.fill_bytes(&mut seed);
        Self {
            hub: hub.to_string(),
            username: account.username.clone(),
            account: account.id,
            session,
            device_key: key_to_hex(&seed),
            certificate: None,
            signature: None,
            expires_at: None,
        }
    }

    /// The device key, `None` when the seed of the file is not 32 bytes of hex.
    pub fn signing_key(&self) -> Option<SigningKey> {
        Some(SigningKey::from_bytes(&key_from_hex(&self.device_key)?))
    }

    /// Whether the game should ask the hub for a certificate: it has none, or half of its life
    /// has passed at `now` (Unix seconds).
    pub fn needs_certificate(&self, now: u64) -> bool {
        match (&self.certificate, &self.signature, self.expires_at) {
            (Some(_), Some(_), Some(expires_at)) => {
                now >= expires_at.saturating_sub(CERTIFICATE_LIFETIME_SECS / 2)
            }
            _ => true,
        }
    }

    /// Keeps the certificate the hub sent; `false`, and nothing changed, when it is not well
    /// formed.
    pub fn set_certificate(&mut self, response: &CertificateResponse) -> bool {
        if response.signed_certificate().is_none() {
            return false;
        }
        self.certificate = Some(response.certificate.clone());
        self.signature = Some(response.signature.clone());
        self.expires_at = Some(response.expires_at);
        true
    }

    /// `None` when there is no file. A file that cannot be read is an error.
    pub fn load(path: &Path) -> io::Result<Option<Self>> {
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(err),
        };
        ron::from_str(&text)
            .map(Some)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))
    }

    /// Writes the file atomically, with mode 0600 on Unix.
    pub fn save(&self, path: &Path) -> io::Result<()> {
        let text =
            ron::ser::to_string_pretty(self, PrettyConfig::new()).map_err(io::Error::other)?;
        write_atomically(path, text.as_bytes(), true)
    }

    /// Deletes the file, as logging out does. Fine when there is none.
    pub fn forget(path: &Path) -> io::Result<()> {
        match fs::remove_file(path) {
            Err(err) if err.kind() != io::ErrorKind::NotFound => Err(err),
            _ => Ok(()),
        }
    }
}

/// The time now in Unix seconds.
pub fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn account() -> AccountSummary {
        AccountSummary {
            id: Uuid::from_u128(0x0102030405060708_090a0b0c0d0e0f10),
            username: "Alex".into(),
        }
    }

    fn certified(expires_at: u64) -> AccountFile {
        AccountFile {
            certificate: Some("AQID".into()),
            signature: Some("AQID".into()),
            expires_at: Some(expires_at),
            ..AccountFile::new("https://hub.test", &account(), "token".into())
        }
    }

    #[test]
    fn the_file_round_trips_and_is_private_to_its_owner() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("account.ron");
        let file = certified(1_000_000);

        file.save(&path).unwrap();

        assert_eq!(AccountFile::load(&path).unwrap(), Some(file));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        assert!(!dir.path().join("account.ron.tmp").exists());
    }

    #[test]
    fn saving_over_a_file_with_wider_rights_leaves_it_private() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("account.ron");
        fs::write(&path, "old").unwrap();
        fs::write(dir.path().join("account.ron.tmp"), "stale").unwrap();

        AccountFile::new("h", &account(), "t".into())
            .save(&path)
            .unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o077, 0);
        }
    }

    #[test]
    fn a_missing_file_is_no_account_and_a_broken_one_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("account.ron");
        assert_eq!(AccountFile::load(&path).unwrap(), None);
        fs::write(&path, "(not an account").unwrap();
        assert_eq!(
            AccountFile::load(&path).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
    }

    #[test]
    fn forgetting_removes_the_file_and_is_fine_without_one() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("account.ron");
        AccountFile::new("h", &account(), "t".into())
            .save(&path)
            .unwrap();
        AccountFile::forget(&path).unwrap();
        assert!(!path.exists());
        AccountFile::forget(&path).unwrap();
    }

    #[test]
    fn the_debug_output_hides_the_session_and_the_device_key() {
        let file = AccountFile::new("https://hub.test", &account(), "secret-token".into());
        let shown = format!(
            "{file:?} {:?}",
            crate::account::Account::LoggedIn(file.clone())
        );
        assert!(!shown.contains("secret-token"), "{shown}");
        assert!(!shown.contains(&file.device_key), "{shown}");
        assert!(shown.contains("Alex"), "{shown}");
    }

    #[test]
    fn every_account_gets_its_own_device_key() {
        let one = AccountFile::new("h", &account(), "t".into());
        let two = AccountFile::new("h", &account(), "t".into());
        assert_ne!(one.device_key, two.device_key);
        assert!(one.signing_key().is_some());
        let broken = AccountFile {
            device_key: "zz".into(),
            ..one
        };
        assert!(broken.signing_key().is_none());
    }

    #[test]
    fn a_certificate_is_renewed_at_half_of_its_life() {
        let week = CERTIFICATE_LIFETIME_SECS;
        let expires_at = 10 * week;
        let file = certified(expires_at);
        let half = expires_at - week / 2;

        assert!(!file.needs_certificate(expires_at - week));
        assert!(!file.needs_certificate(half - 1));
        assert!(file.needs_certificate(half));
        assert!(file.needs_certificate(expires_at + 1));
    }

    #[test]
    fn a_missing_or_partial_certificate_is_renewed_at_once() {
        let mut file = certified(u64::MAX);
        assert!(!file.needs_certificate(0));
        file.signature = None;
        assert!(file.needs_certificate(0));
        file.certificate = None;
        file.expires_at = None;
        assert!(file.needs_certificate(0));
    }

    #[test]
    fn a_certificate_that_is_not_base64_is_not_kept() {
        let mut file = AccountFile::new("h", &account(), "t".into());
        let bad = CertificateResponse {
            certificate: "!!".into(),
            signature: "AQID".into(),
            expires_at: 5,
        };
        assert!(!file.set_certificate(&bad));
        assert_eq!(file.certificate, None);
        let good = CertificateResponse {
            certificate: "AQID".into(),
            signature: "A".repeat(86) + "==",
            expires_at: 5,
        };
        assert!(file.set_certificate(&good));
        assert_eq!(file.expires_at, Some(5));
    }
}
