//! What a server in hub mode learns from its hub: the keys that sign the certificates, read at
//! start and kept in the game folder for when the hub cannot be reached, and the revoked
//! certificates, read at start and every hour.

use std::collections::HashSet;
use std::path::Path;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use bevy::log::{info, warn};
use ed25519_dalek::VerifyingKey;
use rustcraft_protocol::api::{HubKey, Revoked, WellKnown};
use rustcraft_protocol::identity::key_from_hex;
use serde::{Deserialize, Serialize};
use shared::HubTrust;
use uuid::Uuid;

use crate::world::format::write_atomically;

/// The keys of the hub last read, in the game folder.
pub const HUB_KEYS_FILE: &str = "hub_keys.ron";

/// How long a call to the hub may take.
const TIMEOUT: Duration = Duration::from_secs(10);
/// How often the revoked certificates are read again.
const REVOCATIONS_EVERY: Duration = Duration::from_secs(3600);

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
/// `HUB_KEYS_FILE` of `folder`; those of the file when the hub cannot be reached. An error
/// when neither gives any: the server cannot check a player. No certificate is revoked yet
/// (`watch_revocations`).
pub fn hub_trust(url: &str, folder: &Path) -> Result<HubTrust, String> {
    let url = url.trim_end_matches('/').to_string();
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
    Ok(known.keys)
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

/// Reads the revoked certificates of the hub of `trust` now, then every hour in a thread of
/// its own, for the life of the process. A failed read keeps the list of the last one.
pub fn watch_revocations(trust: &HubTrust) {
    let (url, revoked) = (trust.url.clone(), trust.revoked.clone());
    refresh_revocations(&url, &revoked);
    std::thread::spawn(move || loop {
        std::thread::sleep(REVOCATIONS_EVERY);
        refresh_revocations(&url, &revoked);
    });
}

fn refresh_revocations(url: &str, revoked: &RwLock<HashSet<Uuid>>) {
    let answer: Result<Revoked, _> = agent()
        .get(format!("{url}/v1/revoked"))
        .call()
        .and_then(|mut response| response.body_mut().read_json());
    match answer {
        Ok(answer) => {
            if let Ok(mut revoked) = revoked.write() {
                *revoked = answer.ids.into_iter().collect();
            }
        }
        Err(err) => warn!("Could not read the revoked certificates of the hub at {url}: {err}"),
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
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        std::thread::spawn(move || {
            for body in bodies {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = [0u8; 4096];
                let _ = stream.read(&mut request);
                let response = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
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
        let trust = hub_trust(&format!("{url}/"), dir.path()).unwrap();
        assert_eq!(trust.url, url);
        assert_eq!(trust.keys, vec![(1, key())]);
        assert!(dir.path().join(HUB_KEYS_FILE).is_file());

        // the hub is gone: the keys come from the file
        let cache = std::fs::read_to_string(dir.path().join(HUB_KEYS_FILE)).unwrap();
        std::fs::write(dir.path().join(HUB_KEYS_FILE), cache.replace(&url, NOWHERE)).unwrap();
        let trust = hub_trust(NOWHERE, dir.path()).unwrap();
        assert_eq!(trust.keys, vec![(1, key())]);
    }

    #[test]
    fn a_server_without_its_hub_nor_its_keys_does_not_start() {
        let dir = tempfile::tempdir().unwrap();
        assert!(hub_trust(NOWHERE, dir.path()).is_err());
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
        assert!(hub_trust(NOWHERE, dir.path()).is_err());
    }

    #[test]
    fn the_revoked_certificates_are_read_and_kept_when_the_hub_fails() {
        let id = Uuid::from_u128(9);
        let url = hub(vec![format!(r#"{{"ids":["{id}"]}}"#), "broken".into()]);
        let revoked = RwLock::new(HashSet::new());
        refresh_revocations(&url, &revoked);
        assert!(revoked.read().unwrap().contains(&id));
        refresh_revocations(&url, &revoked);
        assert!(revoked.read().unwrap().contains(&id));
        refresh_revocations(NOWHERE, &revoked);
        assert!(revoked.read().unwrap().contains(&id));
    }
}
