//! What a server in hub mode learns from its hub: the keys that sign the certificates, read at
//! start and kept in the game folder for when the hub cannot be reached, and the revoked
//! certificates, read at start and every hour, and the accounts of the names of the players the
//! operators name.

use std::collections::HashSet;
use std::path::Path;
use std::sync::{Arc, PoisonError, RwLock};
use std::time::Duration;

use bevy::log::{info, warn};
use ed25519_dalek::VerifyingKey;
use rustcraft_protocol::api::{HubKey, Profile, Revoked, WellKnown};
use rustcraft_protocol::hub_base;
use rustcraft_protocol::identity::key_from_hex;
use serde::{Deserialize, Serialize};
use shared::HubTrust;
use uuid::Uuid;

use crate::network::permissions::Entry;
use crate::world::format::write_atomically;

/// The keys of the hub last read, in the game folder.
pub const HUB_KEYS_FILE: &str = "hub_keys.ron";

/// How long a call to the hub may take.
const TIMEOUT: Duration = Duration::from_secs(10);
/// How often the revoked certificates are read again.
const REVOCATIONS_EVERY: Duration = Duration::from_secs(3600);
/// How soon they are read again after a failed read.
const REVOCATIONS_RETRY: Duration = Duration::from_secs(5 * 60);

#[derive(Serialize, Deserialize)]
struct CachedKeys {
    /// The hub they are the keys of.
    hub: String,
    keys: Vec<HubKey>,
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(TIMEOUT))
        .build()
        .into()
}

/// The keys of the hub at `url`, read from its well-known document and kept in
/// `HUB_KEYS_FILE` of `folder`; those of the file when the hub cannot be reached or sends no
/// valid key. An error when neither gives any: the server cannot check a player. `addresses`
/// are those the players reach the server at, normalized. No certificate is revoked yet
/// (`watch_revocations`).
pub fn hub_trust(url: &str, addresses: Vec<String>, folder: &Path) -> Result<HubTrust, String> {
    let url = hub_base(url).to_string();
    let path = folder.join(HUB_KEYS_FILE);
    let keys = match fetch_keys(&url) {
        Ok(keys) => {
            let cache = CachedKeys {
                hub: url.clone(),
                keys: keys.clone(),
            };
            let saved = ron::ser::to_string_pretty(&cache, Default::default())
                .map_err(std::io::Error::other)
                .and_then(|text| write_atomically(&path, text.as_bytes()));
            if let Err(err) = saved {
                warn!(
                    "Could not keep the keys of the hub in {}: {err}",
                    path.display()
                );
            }
            keys
        }
        Err(err) => {
            let cached = std::fs::read_to_string(&path)
                .ok()
                .and_then(|text| ron::from_str::<CachedKeys>(&text).ok())
                .filter(|cache| cache.hub == url);
            let Some(cache) = cached else {
                return Err(format!(
                    "Could not read the keys of the hub at {url} ({err}), and {} holds none of it: the players cannot be checked",
                    path.display()
                ));
            };
            warn!(
                "Could not reach the hub at {url} ({err}): using its keys of {}",
                path.display()
            );
            cache.keys
        }
    };
    let keys = verifying_keys(&keys);
    if keys.is_empty() {
        return Err(format!("The hub at {url} has no valid key"));
    }
    info!("Players join with their account of the hub at {url}");
    Ok(HubTrust {
        url,
        addresses,
        keys,
        revoked: Arc::new(RwLock::new(HashSet::new())),
    })
}

fn fetch_keys(url: &str) -> Result<Vec<HubKey>, String> {
    let known: WellKnown = agent()
        .get(format!("{url}/.well-known/rustcraft-hub"))
        .call()
        .and_then(|mut response| response.body_mut().read_json())
        .map_err(|err| err.to_string())?;
    // Only the valid keys are kept, in the file too
    let keys: Vec<HubKey> = known
        .keys
        .into_iter()
        .filter(|key| !verifying_keys(std::slice::from_ref(key)).is_empty())
        .collect();
    if keys.is_empty() {
        return Err("its well-known document has no valid key".into());
    }
    Ok(keys)
}

/// The keys of the hub that are 32 bytes of hex and valid points, with their ids.
fn verifying_keys(keys: &[HubKey]) -> Vec<(u8, VerifyingKey)> {
    keys.iter()
        .filter_map(|key| {
            let bytes = key_from_hex(&key.public_key)?;
            match VerifyingKey::from_bytes(&bytes) {
                Ok(verifying) => Some((key.id, verifying)),
                Err(_) => {
                    warn!("Ignoring the key {} of the hub: not a valid key", key.id);
                    None
                }
            }
        })
        .collect()
}

/// The account named `name` on the hub at `url`: `None` when the hub does not know it.
pub fn fetch_profile(url: &str, name: &str) -> Result<Option<Profile>, String> {
    let answer = agent()
        .get(format!("{url}/v1/profiles"))
        .query("name", name)
        .call();
    match answer {
        Ok(mut response) => response
            .body_mut()
            .read_json()
            .map(Some)
            .map_err(|err| err.to_string()),
        Err(ureq::Error::StatusCode(404)) => Ok(None),
        Err(err) => Err(err.to_string()),
    }
}

/// The accounts of `names` on the hub at `url`, for the `--op` option: an error naming the first
/// that the hub does not know, or when it cannot be reached.
pub fn resolve_names(url: &str, names: &[String]) -> Result<Vec<Entry>, String> {
    names
        .iter()
        .map(|name| match fetch_profile(url, name) {
            Ok(Some(profile)) => Ok(Entry::account(profile.id, profile.username)),
            Ok(None) => Err(format!(
                "The hub at {url} does not know a player named {name}"
            )),
            Err(err) => Err(format!("Could not ask the hub at {url} for {name}: {err}")),
        })
        .collect()
}

/// Reads the revoked certificates of the hub of `trust` now, then in a thread of its own for
/// the life of the process: every hour, or every 5 minutes after a failed read until one
/// succeeds. A failed read keeps the list of the last one.
pub fn watch_revocations(trust: &HubTrust) {
    let (url, revoked) = (trust.url.clone(), trust.revoked.clone());
    let mut read = refresh_revocations(&url, &revoked);
    std::thread::spawn(move || loop {
        std::thread::sleep(if read {
            REVOCATIONS_EVERY
        } else {
            REVOCATIONS_RETRY
        });
        read = refresh_revocations(&url, &revoked);
    });
}

/// Whether the list could be read.
fn refresh_revocations(url: &str, revoked: &RwLock<HashSet<Uuid>>) -> bool {
    let answer: Result<Revoked, _> = agent()
        .get(format!("{url}/v1/revoked"))
        .call()
        .and_then(|mut response| response.body_mut().read_json());
    match answer {
        Ok(answer) => {
            *revoked.write().unwrap_or_else(PoisonError::into_inner) =
                answer.ids.into_iter().collect();
            true
        }
        Err(err) => {
            warn!("Could not read the revoked certificates of the hub at {url}: {err}");
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustcraft_protocol::identity::key_to_hex;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    /// A hub that answers each of its first connections with the next of `bodies` (JSON, 200).
    fn hub(bodies: Vec<String>) -> String {
        hub_answering(bodies.into_iter().map(|body| ("200 OK", body)).collect())
    }

    /// A hub that answers each of its first connections with the next of `answers`: a status
    /// line and a JSON body.
    fn hub_answering(answers: Vec<(&'static str, String)>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        std::thread::spawn(move || {
            for (status, body) in answers {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = [0u8; 4096];
                let _ = stream.read(&mut request);
                let response = format!(
                    "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
                stream.write_all(response.as_bytes()).unwrap();
            }
        });
        url
    }

    /// A URL where nothing answers.
    const NOWHERE: &str = "http://127.0.0.1:1";

    fn key() -> VerifyingKey {
        ed25519_dalek::SigningKey::from_bytes(&[5; 32]).verifying_key()
    }

    fn well_known() -> String {
        format!(
            r#"{{"name":"Test","url":"https://hub.example","api_version":1,"keys":[{{"id":1,"public_key":"{}"}}],"protocol_id":1}}"#,
            key_to_hex(&key().to_bytes())
        )
    }

    #[test]
    fn the_keys_of_the_hub_are_kept_for_when_it_cannot_be_reached() {
        let dir = tempfile::tempdir().unwrap();
        let url = hub(vec![well_known()]);
        let trust = hub_trust(&format!("{url}/"), vec![], dir.path()).unwrap();
        assert_eq!(trust.url, url);
        assert_eq!(trust.keys, vec![(1, key())]);
        assert!(dir.path().join(HUB_KEYS_FILE).is_file());

        // the hub is gone: the keys come from the file
        let cache = std::fs::read_to_string(dir.path().join(HUB_KEYS_FILE)).unwrap();
        std::fs::write(dir.path().join(HUB_KEYS_FILE), cache.replace(&url, NOWHERE)).unwrap();
        let trust = hub_trust(NOWHERE, vec![], dir.path()).unwrap();
        assert_eq!(trust.keys, vec![(1, key())]);
    }

    #[test]
    fn only_valid_keys_are_kept_and_a_hub_without_any_is_not_trusted() {
        let dir = tempfile::tempdir().unwrap();
        let mixed = well_known().replace(
            r#"}],"protocol_id""#,
            r#"},{"id":2,"public_key":"zz"}],"protocol_id""#,
        );
        let trust = hub_trust(&hub(vec![mixed]), vec![], dir.path()).unwrap();
        assert_eq!(trust.keys, vec![(1, key())]);
        let cache = std::fs::read_to_string(dir.path().join(HUB_KEYS_FILE)).unwrap();
        assert!(!cache.contains("zz"), "{cache}");

        let empty = tempfile::tempdir().unwrap();
        let invalid = well_known().replace(&key_to_hex(&key().to_bytes()), "zz");
        assert!(hub_trust(&hub(vec![invalid]), vec![], empty.path()).is_err());
        assert!(!empty.path().join(HUB_KEYS_FILE).exists());
    }

    #[test]
    fn a_server_without_its_hub_nor_its_keys_does_not_start() {
        let dir = tempfile::tempdir().unwrap();
        assert!(hub_trust(NOWHERE, vec![], dir.path()).is_err());
        // the keys of another hub are not trusted
        let cache = CachedKeys {
            hub: "https://other.example".into(),
            keys: vec![HubKey {
                id: 1,
                public_key: key_to_hex(&key().to_bytes()),
            }],
        };
        let text = ron::ser::to_string(&cache).unwrap();
        std::fs::write(dir.path().join(HUB_KEYS_FILE), text).unwrap();
        assert!(hub_trust(NOWHERE, vec![], dir.path()).is_err());
    }

    #[test]
    fn the_revoked_certificates_are_read_and_kept_when_the_hub_fails() {
        let id = Uuid::from_u128(9);
        let url = hub(vec![format!(r#"{{"ids":["{id}"]}}"#), "broken".into()]);
        let revoked = RwLock::new(HashSet::new());
        assert!(refresh_revocations(&url, &revoked));
        assert!(revoked.read().unwrap().contains(&id));
        assert!(!refresh_revocations(&url, &revoked));
        assert!(revoked.read().unwrap().contains(&id));
        assert!(!refresh_revocations(NOWHERE, &revoked));
        assert!(revoked.read().unwrap().contains(&id));
    }

    #[test]
    fn the_account_of_a_name_is_asked_to_the_hub() {
        let id = Uuid::from_u128(9);
        let url = hub_answering(vec![
            ("200 OK", format!(r#"{{"id":"{id}","username":"Bob"}}"#)),
            ("404 Not Found", r#"{"error":"not_found"}"#.into()),
            ("500 Internal Server Error", "{}".into()),
        ]);
        let profile = fetch_profile(&url, "bob").unwrap().unwrap();
        assert_eq!((profile.id, profile.username.as_str()), (id, "Bob"));
        assert_eq!(fetch_profile(&url, "nobody"), Ok(None));
        assert!(fetch_profile(&url, "bob").is_err());
        assert!(fetch_profile(NOWHERE, "bob").is_err());
    }

    #[test]
    fn the_names_of_the_operators_are_all_accounts_of_the_hub_or_an_error() {
        let id = Uuid::from_u128(9);
        let known = || ("200 OK", format!(r#"{{"id":"{id}","username":"Bob"}}"#));
        let unknown = || ("404 Not Found", "{}".to_string());
        let names = ["bob".to_string(), "eve".to_string()];
        let url = hub_answering(vec![known(), known()]);
        let entries = resolve_names(&url, &names).unwrap();
        assert_eq!(
            entries,
            [Entry::account(id, "Bob"), Entry::account(id, "Bob")]
        );
        let url = hub_answering(vec![known(), unknown()]);
        let err = resolve_names(&url, &names).unwrap_err();
        assert!(err.contains("does not know a player named eve"), "{err}");
        assert!(resolve_names(NOWHERE, &names).is_err());
    }
}
