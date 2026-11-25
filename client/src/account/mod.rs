//! The account of the player on the hub: `account.ron`, the calls to the hub, the certificate
//! kept fresh in the background, and the name and id the account gives the player. The Account
//! screen and the join handshake build on it.

pub mod file;
pub mod hub;
#[cfg(test)]
pub mod test_hub;

use std::time::{Duration, Instant};

use bevy::prelude::*;
use rustcraft_protocol::api::CertificateResponse;
use rustcraft_protocol::identity::sign_join;
use shared::messages::{JoinProof, PlayerId};
use shared::GameFolderPaths;
use uuid::Uuid;

use crate::constants::ACCOUNT_PATH;
use crate::network::status::InFlight;
use crate::network::{hash_string_to_u64, CurrentPlayerProfile};
use crate::ui::menus::player_name::load_player_name;

use file::{unix_now, AccountFile};
use hub::{HubClient, HubError};

/// How often the game looks whether its certificate is due for renewal, the first look being at
/// start.
const RENEWAL_CHECK_EVERY: Duration = Duration::from_secs(10 * 60);
/// How long a request for a certificate may be waited for. The hub client gives up after 10 s,
/// so a request still there after this one has lost its thread (a panic), and a new one may go.
const RENEWAL_DEADLINE: Duration = Duration::from_secs(30);

/// Whether the player is signed in to a hub.
#[derive(Resource, Clone, Debug, PartialEq, Eq)]
pub enum Account {
    LoggedOut,
    LoggedIn(AccountFile),
}

impl Account {
    /// What `account.ron` holds. A file that cannot be read counts as no account, with a
    /// warning.
    pub fn load(paths: &GameFolderPaths) -> Self {
        let path = paths.game_folder_path.join(ACCOUNT_PATH);
        match AccountFile::load(&path) {
            Ok(Some(file)) => Account::LoggedIn(file),
            Ok(None) => Account::LoggedOut,
            Err(e) => {
                warn!("Cannot read {path:?}, playing without an account: {e}");
                Account::LoggedOut
            }
        }
    }

    /// What proves the account to a server of `hub` that gave this challenge: its certificate
    /// and the signature of the device. `None` when logged out, on another hub, or without a
    /// certificate yet.
    pub fn join_proof(
        &self,
        hub: &str,
        nonce: &[u8; 32],
        server_key: &[u8; 32],
    ) -> Option<JoinProof> {
        let Account::LoggedIn(file) = self else {
            return None;
        };
        if file.hub.trim_end_matches('/') != hub.trim_end_matches('/') {
            return None;
        }
        let signed = CertificateResponse {
            certificate: file.certificate.clone()?,
            signature: file.signature.clone()?,
            expires_at: file.expires_at.unwrap_or_default(),
        }
        .signed_certificate()?;
        let device = file.signing_key()?;
        Some(JoinProof {
            certificate: signed.bytes,
            certificate_signature: signed.signature.to_vec(),
            signature: sign_join(&device, nonce, server_key).to_vec(),
        })
    }

    /// The name and the id the account gives the player, when signed in.
    pub fn profile(&self) -> Option<(String, PlayerId)> {
        match self {
            Account::LoggedIn(file) => Some((file.username.clone(), player_id(&file.account))),
            Account::LoggedOut => None,
        }
    }
}

/// The hub of the `--hub` flag: the one an account is made on or logged in to.
#[derive(Resource, Clone, Debug, PartialEq, Eq)]
pub struct HubUrl(pub String);

/// Present when the name of the player was given on the command line, which wins over the one
/// of the account.
#[derive(Resource)]
pub struct ExplicitPlayerName;

pub use rustcraft_protocol::identity::player_id;

pub fn account_plugin(app: &mut App) {
    let mut timer = Timer::new(RENEWAL_CHECK_EVERY, TimerMode::Repeating);
    timer.set_elapsed(RENEWAL_CHECK_EVERY);
    app.insert_resource(Renewal {
        timer,
        in_flight: None,
    })
    .add_systems(
        Update,
        (
            renew_certificate,
            follow_account.run_if(
                resource_changed::<Account>.and_then(not(resource_exists::<ExplicitPlayerName>)),
            ),
        ),
    );
}

type RenewalAnswer = (Uuid, Result<CertificateResponse, HubError>);

#[derive(Resource)]
struct Renewal {
    timer: Timer,
    /// The request, with the time it was sent.
    in_flight: Option<(Instant, InFlight<RenewalAnswer>)>,
}

/// Asks the hub for a certificate when the account has none or half of its life has passed,
/// at start and every RENEWAL_CHECK_EVERY, and keeps it in `account.ron`. The old certificate
/// stays when the hub cannot be reached or refuses.
fn renew_certificate(
    time: Res<Time>,
    mut renewal: ResMut<Renewal>,
    mut account: ResMut<Account>,
    paths: Res<GameFolderPaths>,
) {
    if let Some((sent, request)) = &renewal.in_flight {
        if let Some(answer) = request.take() {
            renewal.in_flight = None;
            keep_certificate(&mut account, &paths, answer);
        } else if sent.elapsed() > RENEWAL_DEADLINE {
            warn!("The request for a certificate never came back, forgetting it");
            renewal.in_flight = None;
        }
    }
    if !renewal.timer.tick(time.delta()).just_finished() || renewal.in_flight.is_some() {
        return;
    }
    let Account::LoggedIn(file) = &*account else {
        return;
    };
    if !file.needs_certificate(unix_now()) {
        return;
    }
    let Some(key) = file.signing_key() else {
        warn!("The device key of account.ron is not valid: no certificate can be asked");
        return;
    };
    let (client, session, id) = (
        HubClient::new(&file.hub),
        file.session.clone(),
        file.account,
    );
    let public_key = key.verifying_key().to_bytes();
    renewal.in_flight = Some((
        Instant::now(),
        InFlight::spawn(
            (id, Err(HubError::Unreachable("no thread to ask on".into()))),
            move || (id, client.certificate(&session, &public_key)),
        ),
    ));
}

fn keep_certificate(account: &mut Account, paths: &GameFolderPaths, answer: RenewalAnswer) {
    let (id, result) = answer;
    let response = match result {
        Ok(response) => response,
        Err(e) => {
            warn!("Cannot renew the certificate, keeping the old one: {e}");
            return;
        }
    };
    // The player may have logged out, or into another account, while the hub answered
    let Account::LoggedIn(file) = account else {
        return;
    };
    if file.account != id {
        return;
    }
    let mut renewed = file.clone();
    if !renewed.set_certificate(&response) {
        warn!("The hub sent a certificate that is not well formed, keeping the old one");
        return;
    }
    if let Err(e) = renewed.save(&paths.game_folder_path.join(ACCOUNT_PATH)) {
        warn!("Cannot save the certificate in account.ron: {e}");
    }
    *file = renewed;
}

/// Makes the account the player: its username and its id. Logged out, the saved name comes
/// back, when there is one.
fn follow_account(
    account: Res<Account>,
    paths: Res<GameFolderPaths>,
    mut profile: ResMut<CurrentPlayerProfile>,
) {
    let (name, id) = match account.profile() {
        Some(identity) => identity,
        None => match load_player_name(&paths) {
            Some(name) => {
                let id = hash_string_to_u64(&name);
                (name, id)
            }
            None => return,
        },
    };
    if profile.name != name || profile.id != id {
        *profile = CurrentPlayerProfile { id, name };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::test_hub::{Canned, TestHub};
    use rustcraft_protocol::api::AccountSummary;
    use rustcraft_protocol::identity::CERTIFICATE_LIFETIME_SECS;

    const UUID: u128 = 0x0102030405060708_090a0b0c0d0e0f10;

    fn summary() -> AccountSummary {
        AccountSummary {
            id: Uuid::from_u128(UUID),
            username: "Alex".into(),
        }
    }

    fn paths(dir: &tempfile::TempDir) -> GameFolderPaths {
        GameFolderPaths {
            game_folder_path: dir.path().to_path_buf(),
            assets_folder_path: dir.path().to_path_buf(),
        }
    }

    #[test]
    fn a_logged_in_account_gives_its_username_and_id_and_a_logged_out_one_nothing() {
        let account = Account::LoggedIn(AccountFile::new("h", &summary(), "t".into()));
        assert_eq!(
            account.profile(),
            Some(("Alex".to_string(), player_id(&Uuid::from_u128(UUID))))
        );
        assert_eq!(Account::LoggedOut.profile(), None);
    }

    #[test]
    fn a_join_proof_is_made_for_the_hub_of_the_account_only() {
        let (nonce, server_key) = ([1u8; 32], [2u8; 32]);
        let mut file = AccountFile::new("https://hub.example", &summary(), "t".into());
        let without_certificate = Account::LoggedIn(file.clone());
        assert_eq!(
            without_certificate.join_proof("https://hub.example", &nonce, &server_key),
            None
        );
        file.certificate = Some("AQID".into());
        file.signature = Some(format!("{}==", "A".repeat(86)));
        let device = file.signing_key().unwrap();
        let account = Account::LoggedIn(file);

        let proof = account
            .join_proof("https://hub.example/", &nonce, &server_key)
            .unwrap();
        assert_eq!(proof.certificate, vec![1, 2, 3]);
        assert_eq!(proof.certificate_signature, vec![0; 64]);
        assert_eq!(
            proof.signature,
            sign_join(&device, &nonce, &server_key).to_vec()
        );
        assert_eq!(
            account.join_proof("https://other.example", &nonce, &server_key),
            None
        );
        assert_eq!(
            Account::LoggedOut.join_proof("https://hub.example", &nonce, &server_key),
            None
        );
    }

    #[test]
    fn the_account_loads_from_the_game_folder() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(Account::load(&paths(&dir)), Account::LoggedOut);
        let file = AccountFile::new("h", &summary(), "t".into());
        file.save(&dir.path().join(ACCOUNT_PATH)).unwrap();
        assert_eq!(Account::load(&paths(&dir)), Account::LoggedIn(file));
        std::fs::write(dir.path().join(ACCOUNT_PATH), "(broken").unwrap();
        assert_eq!(Account::load(&paths(&dir)), Account::LoggedOut);
    }

    fn app_with(account: Account, dir: &tempfile::TempDir) -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(account)
            .insert_resource(paths(dir))
            .insert_resource(CurrentPlayerProfile {
                id: 1,
                name: "Player".into(),
            })
            .add_plugins(account_plugin);
        app
    }

    #[test]
    fn the_account_becomes_the_player_and_logging_out_gives_the_name_back() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("player.ron"), "(name: \"Steve\")").unwrap();
        let mut app = app_with(
            Account::LoggedIn(AccountFile::new("h", &summary(), "t".into())),
            &dir,
        );

        app.update();
        let profile = app.world().resource::<CurrentPlayerProfile>();
        assert_eq!(profile.name, "Alex");
        assert_eq!(profile.id, player_id(&Uuid::from_u128(UUID)));

        *app.world_mut().resource_mut::<Account>() = Account::LoggedOut;
        app.update();
        let profile = app.world().resource::<CurrentPlayerProfile>();
        assert_eq!(profile.name, "Steve");
        assert_eq!(profile.id, hash_string_to_u64("Steve"));
    }

    #[test]
    fn an_explicit_player_name_wins_over_the_account() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = app_with(
            Account::LoggedIn(AccountFile::new(
                "http://127.0.0.1:1",
                &summary(),
                "t".into(),
            )),
            &dir,
        );
        app.insert_resource(ExplicitPlayerName);
        app.update();
        assert_eq!(
            app.world().resource::<CurrentPlayerProfile>().name,
            "Player"
        );
    }

    fn run_until(app: &mut App, done: impl Fn(&App) -> bool) {
        let start = Instant::now();
        while !done(app) {
            assert!(start.elapsed() < Duration::from_secs(10), "never happened");
            app.update();
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn certificate_json(expires_at: u64) -> String {
        format!(
            r#"{{"certificate":"AQID","signature":"{}==","expires_at":{expires_at}}}"#,
            "A".repeat(86)
        )
    }

    #[test]
    fn a_certificate_is_asked_at_start_and_kept_in_the_file() {
        let expires_at = unix_now() + CERTIFICATE_LIFETIME_SECS;
        let hub = TestHub::start(vec![Canned::ok(&certificate_json(expires_at))]);
        let dir = tempfile::tempdir().unwrap();
        let file = AccountFile::new(&hub.url, &summary(), "tok".into());
        let public_key = file.signing_key().unwrap().verifying_key().to_bytes();
        let mut app = app_with(Account::LoggedIn(file), &dir);

        run_until(&mut app, |app| {
            matches!(app.world().resource::<Account>(),
                Account::LoggedIn(file) if file.expires_at == Some(expires_at))
        });

        let saved = AccountFile::load(&dir.path().join(ACCOUNT_PATH))
            .unwrap()
            .unwrap();
        assert_eq!(saved.expires_at, Some(expires_at));
        let request = &hub.requests()[0];
        assert_eq!(request.path, "/v1/certificates");
        assert_eq!(request.header("authorization"), Some("Bearer tok"));
        assert!(request
            .body
            .contains(&rustcraft_protocol::identity::key_to_hex(&public_key)));
    }

    #[test]
    fn the_old_certificate_stays_when_the_hub_refuses() {
        let hub = TestHub::start(vec![Canned::error(429, "rate_limited", "Slow down.")]);
        let dir = tempfile::tempdir().unwrap();
        let mut file = AccountFile::new(&hub.url, &summary(), "tok".into());
        file.certificate = Some("AQID".into());
        file.signature = Some("AQID".into());
        file.expires_at = Some(unix_now() + 1);
        let mut app = app_with(Account::LoggedIn(file.clone()), &dir);

        // The answer has been read once the request is no longer in flight
        run_until(&mut app, |app| {
            hub.requests().len() == 1 && app.world().resource::<Renewal>().in_flight.is_none()
        });

        assert_eq!(*app.world().resource::<Account>(), Account::LoggedIn(file));
    }

    #[test]
    fn a_fresh_certificate_is_not_asked_again() {
        let hub = TestHub::start(vec![Canned::ok("{}")]);
        let dir = tempfile::tempdir().unwrap();
        let mut file = AccountFile::new(&hub.url, &summary(), "tok".into());
        file.certificate = Some("AQID".into());
        file.signature = Some("AQID".into());
        file.expires_at = Some(unix_now() + CERTIFICATE_LIFETIME_SECS);
        let mut app = app_with(Account::LoggedIn(file), &dir);

        for _ in 0..10 {
            app.update();
            std::thread::sleep(Duration::from_millis(10));
        }

        assert!(hub.requests().is_empty());
    }

    #[test]
    fn a_request_that_lost_its_thread_does_not_block_the_next_one() {
        let expires_at = unix_now() + CERTIFICATE_LIFETIME_SECS;
        let hub = TestHub::start(vec![Canned::ok(&certificate_json(expires_at))]);
        let dir = tempfile::tempdir().unwrap();
        let mut app = app_with(
            Account::LoggedIn(AccountFile::new(&hub.url, &summary(), "tok".into())),
            &dir,
        );
        // A request whose thread never gives an answer, as one that panicked
        let lost = InFlight::spawn(
            (Uuid::nil(), Err(HubError::Unreachable(String::new()))),
            || {
                std::thread::sleep(Duration::from_secs(20));
                (Uuid::nil(), Err(HubError::Unreachable(String::new())))
            },
        );
        let sent = Instant::now() - RENEWAL_DEADLINE - Duration::from_secs(1);
        app.world_mut().resource_mut::<Renewal>().in_flight = Some((sent, lost));

        run_until(&mut app, |app| {
            matches!(app.world().resource::<Account>(),
                Account::LoggedIn(file) if file.expires_at == Some(expires_at))
        });
    }
}
