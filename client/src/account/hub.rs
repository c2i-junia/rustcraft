//! The calls to the hub. Blocking and free of Bevy: the game runs them on a thread (`InFlight`),
//! and a server may reuse them.

use std::fmt;
use std::time::Duration;

use rustcraft_protocol::api::{
    CertificateRequest, CertificateResponse, CreateRealmRequest, ErrorBody, ErrorCode,
    LoginRequest, Me, MemberRole, Realm, RealmList, RecoveryRequest, SessionResponse,
    SessionWithRecovery, SetMemberRequest, SignUpRequest, StartRealmResponse,
};
use rustcraft_protocol::identity::key_to_hex;
use serde::de::DeserializeOwned;
use uuid::Uuid;

/// How long a call to the hub may take in all.
const TIMEOUT: Duration = Duration::from_secs(10);

/// Why a call to the hub failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HubError {
    /// The hub refused: its error code and its message.
    Api { code: ErrorCode, message: String },
    /// No answer: the connection failed or timed out.
    Unreachable(String),
    /// An answer that is not what the API says.
    Malformed(String),
}

impl HubError {
    /// Whether the session token is not valid (any more): the player has to log in again.
    pub fn is_unauthorized(&self) -> bool {
        matches!(
            self,
            HubError::Api {
                code: ErrorCode::Unauthorized,
                ..
            }
        )
    }
}

impl fmt::Display for HubError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HubError::Api { message, .. } => f.write_str(message),
            HubError::Unreachable(why) => write!(f, "The hub cannot be reached: {why}"),
            HubError::Malformed(why) => {
                write!(f, "The hub sent an answer that makes no sense: {why}")
            }
        }
    }
}

impl std::error::Error for HubError {}

/// A hub, by its base URL.
#[derive(Clone)]
pub struct HubClient {
    base: String,
    agent: ureq::Agent,
}

impl HubClient {
    pub fn new(base: &str) -> Self {
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(TIMEOUT))
            // The error status codes carry an `ErrorBody` that is read like any answer
            .http_status_as_error(false)
            .build()
            .into();
        Self {
            base: base.trim_end_matches('/').to_string(),
            agent,
        }
    }

    /// `POST /v1/accounts`
    pub fn create_account(
        &self,
        username: &str,
        password: &str,
        device_name: Option<&str>,
    ) -> Result<SessionWithRecovery, HubError> {
        let body = SignUpRequest {
            username: username.to_string(),
            password: password.to_string(),
            device_name: device_name.map(str::to_string),
        };
        self.read(self.agent.post(self.url("/v1/accounts")).send_json(&body))
    }

    /// `POST /v1/sessions`
    pub fn log_in(
        &self,
        username: &str,
        password: &str,
        device_name: Option<&str>,
    ) -> Result<SessionResponse, HubError> {
        let body = LoginRequest {
            username: username.to_string(),
            password: password.to_string(),
            device_name: device_name.map(str::to_string),
        };
        self.read(self.agent.post(self.url("/v1/sessions")).send_json(&body))
    }

    /// `POST /v1/recovery`: sets a new password and gives a new recovery code.
    pub fn recover(
        &self,
        username: &str,
        recovery_code: &str,
        new_password: &str,
    ) -> Result<SessionWithRecovery, HubError> {
        let body = RecoveryRequest {
            username: username.to_string(),
            recovery_code: recovery_code.to_string(),
            new_password: new_password.to_string(),
        };
        self.read(self.agent.post(self.url("/v1/recovery")).send_json(&body))
    }

    /// `DELETE /v1/sessions/current`
    pub fn log_out(&self, session: &str) -> Result<(), HubError> {
        let response = self
            .agent
            .delete(self.url("/v1/sessions/current"))
            .header("Authorization", bearer(session))
            .call();
        self.check(response).map(|_| ())
    }

    /// `GET /v1/me`
    pub fn me(&self, session: &str) -> Result<Me, HubError> {
        self.read(
            self.agent
                .get(self.url("/v1/me"))
                .header("Authorization", bearer(session))
                .call(),
        )
    }

    /// `POST /v1/certificates`: a certificate of 7 days for the public `device_key`.
    pub fn certificate(
        &self,
        session: &str,
        device_key: &[u8; 32],
    ) -> Result<CertificateResponse, HubError> {
        let body = CertificateRequest {
            device_key: key_to_hex(device_key),
        };
        self.read(
            self.agent
                .post(self.url("/v1/certificates"))
                .header("Authorization", bearer(session))
                .send_json(&body),
        )
    }

    /// `GET /v1/realms`: the hosted worlds the account owns or is a member of.
    pub fn realms(&self, session: &str) -> Result<RealmList, HubError> {
        self.read(
            self.agent
                .get(self.url("/v1/realms"))
                .header("Authorization", bearer(session))
                .call(),
        )
    }

    /// `POST /v1/realms`: a new hosted world, stopped.
    pub fn create_realm(
        &self,
        session: &str,
        request: &CreateRealmRequest,
    ) -> Result<Realm, HubError> {
        self.read(
            self.agent
                .post(self.url("/v1/realms"))
                .header("Authorization", bearer(session))
                .send_json(request),
        )
    }

    /// `POST /v1/realms/{id}/start`: starts the world if it is stopped, and says where it runs.
    pub fn start_realm(&self, session: &str, id: Uuid) -> Result<StartRealmResponse, HubError> {
        self.read(
            self.agent
                .post(self.url(&format!("/v1/realms/{id}/start")))
                .header("Authorization", bearer(session))
                .send_empty(),
        )
    }

    /// `POST /v1/realms/{id}/stop`: the hub stops the world in the background.
    pub fn stop_realm(&self, session: &str, id: Uuid) -> Result<(), HubError> {
        let response = self
            .agent
            .post(self.url(&format!("/v1/realms/{id}/stop")))
            .header("Authorization", bearer(session))
            .send_empty();
        self.check(response).map(|_| ())
    }

    /// `DELETE /v1/realms/{id}`: archives the world.
    pub fn delete_realm(&self, session: &str, id: Uuid) -> Result<(), HubError> {
        let response = self
            .agent
            .delete(self.url(&format!("/v1/realms/{id}")))
            .header("Authorization", bearer(session))
            .call();
        self.check(response).map(|_| ())
    }

    /// `PUT /v1/realms/{id}/members/{username}`: adds a member, or changes their role.
    pub fn set_member(
        &self,
        session: &str,
        id: Uuid,
        username: &str,
        role: MemberRole,
    ) -> Result<(), HubError> {
        let response = self
            .agent
            .put(self.url(&member_path(id, username)))
            .header("Authorization", bearer(session))
            .send_json(SetMemberRequest { role });
        self.check(response).map(|_| ())
    }

    /// `DELETE /v1/realms/{id}/members/{username}`
    pub fn remove_member(&self, session: &str, id: Uuid, username: &str) -> Result<(), HubError> {
        let response = self
            .agent
            .delete(self.url(&member_path(id, username)))
            .header("Authorization", bearer(session))
            .call();
        self.check(response).map(|_| ())
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base)
    }

    /// The response when its status is a success, the hub's error otherwise.
    fn check(
        &self,
        response: Result<ureq::http::Response<ureq::Body>, ureq::Error>,
    ) -> Result<ureq::http::Response<ureq::Body>, HubError> {
        let mut response = response.map_err(|e| HubError::Unreachable(e.to_string()))?;
        let status = response.status();
        if status.is_success() {
            return Ok(response);
        }
        let text = response.body_mut().read_to_string().unwrap_or_default();
        Err(match serde_json::from_str::<ErrorBody>(&text) {
            Ok(body) => HubError::Api {
                code: body.error,
                message: body.message,
            },
            // A proxy in front of the hub, for one
            Err(_) => HubError::Api {
                code: ErrorCode::Unknown,
                message: format!("The hub answered with the status {status}."),
            },
        })
    }

    fn read<T: DeserializeOwned>(
        &self,
        response: Result<ureq::http::Response<ureq::Body>, ureq::Error>,
    ) -> Result<T, HubError> {
        self.check(response)?
            .body_mut()
            .read_json()
            .map_err(|e| HubError::Malformed(e.to_string()))
    }
}

fn bearer(session: &str) -> String {
    format!("Bearer {session}")
}

/// The path of the member `username` of the world `id`, the name percent-encoded: it is typed by
/// the player.
fn member_path(id: Uuid, username: &str) -> String {
    let mut encoded = String::new();
    for byte in username.bytes() {
        if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
            encoded.push(char::from(byte));
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    format!("/v1/realms/{id}/members/{encoded}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::test_hub::{Canned, TestHub};
    use serde_json::{json, Value};

    const ACCOUNT: &str = r#"{"id":"00000000-0000-0000-0000-000000000001","username":"Alex"}"#;

    #[test]
    fn logging_in_sends_the_credentials_and_reads_the_session() {
        let hub = TestHub::start(vec![Canned::ok(&format!(
            r#"{{"account":{ACCOUNT},"session":"tok"}}"#
        ))]);

        let session = HubClient::new(&hub.url)
            .log_in("Alex", "correct horse", Some("PC"))
            .unwrap();

        assert_eq!(session.session, "tok");
        assert_eq!(session.account.username, "Alex");
        let request = &hub.requests()[0];
        assert_eq!(
            (request.method.as_str(), request.path.as_str()),
            ("POST", "/v1/sessions")
        );
        let body: Value = serde_json::from_str(&request.body).unwrap();
        assert_eq!(
            body,
            json!({"username": "Alex", "password": "correct horse", "device_name": "PC"})
        );
    }

    #[test]
    fn a_trailing_slash_of_the_hub_url_is_harmless() {
        let hub = TestHub::start(vec![Canned::ok(&format!(
            r#"{{"account":{ACCOUNT},"session":"tok","recovery_code":"RCV-1"}}"#
        ))]);

        let created = HubClient::new(&format!("{}/", hub.url))
            .create_account("Alex", "pw", None)
            .unwrap();

        assert_eq!(created.recovery_code, "RCV-1");
        assert_eq!(hub.requests()[0].path, "/v1/accounts");
    }

    #[test]
    fn an_error_body_becomes_the_code_and_the_message() {
        let hub = TestHub::start(vec![Canned::error(
            409,
            "username_taken",
            "That name is taken.",
        )]);

        let error = HubClient::new(&hub.url)
            .create_account("Alex", "pw", None)
            .unwrap_err();

        assert_eq!(
            error,
            HubError::Api {
                code: ErrorCode::UsernameTaken,
                message: "That name is taken.".into()
            }
        );
        assert_eq!(error.to_string(), "That name is taken.");
        assert!(!error.is_unauthorized());
    }

    #[test]
    fn an_unknown_code_and_an_answer_that_is_no_error_body_still_give_a_message() {
        let hub = TestHub::start(vec![
            Canned::error(400, "from_the_future", "Hi."),
            Canned {
                status: 502,
                body: "<html>Bad gateway</html>".into(),
            },
        ]);
        let client = HubClient::new(&hub.url);

        assert_eq!(
            client.me("t").unwrap_err(),
            HubError::Api {
                code: ErrorCode::Unknown,
                message: "Hi.".into()
            }
        );
        let HubError::Api { code, message } = client.me("t").unwrap_err() else {
            panic!("not an API error");
        };
        assert_eq!(code, ErrorCode::Unknown);
        assert!(message.contains("502"), "{message}");
    }

    #[test]
    fn a_session_that_is_no_longer_valid_is_unauthorized() {
        let hub = TestHub::start(vec![Canned::error(401, "unauthorized", "Log in again.")]);
        let error = HubClient::new(&hub.url).me("old").unwrap_err();
        assert!(error.is_unauthorized());
    }

    #[test]
    fn calls_with_a_session_carry_it_as_a_bearer_token() {
        let hub = TestHub::start(vec![
            Canned::ok(
                r#"{"id":"00000000-0000-0000-0000-000000000001","username":"Alex","roles":["host"],"realm_quota":1,"created_at":7}"#,
            ),
            Canned {
                status: 204,
                body: String::new(),
            },
            Canned::ok(r#"{"certificate":"AQID","signature":"AQID","expires_at":9}"#),
        ]);
        let client = HubClient::new(&hub.url);

        let me = client.me("tok").unwrap();
        client.log_out("tok").unwrap();
        let certificate = client.certificate("tok", &[0xab; 32]).unwrap();

        assert_eq!(me.username, "Alex");
        assert_eq!(certificate.expires_at, 9);
        let requests = hub.requests();
        assert_eq!(requests.len(), 3);
        for request in &requests {
            assert_eq!(request.header("authorization"), Some("Bearer tok"));
        }
        assert_eq!(
            (requests[1].method.as_str(), requests[1].path.as_str()),
            ("DELETE", "/v1/sessions/current")
        );
        let body: Value = serde_json::from_str(&requests[2].body).unwrap();
        assert_eq!(body, json!({"device_key": "ab".repeat(32)}));
    }

    #[test]
    fn recovery_sends_the_code_and_the_new_password() {
        let hub = TestHub::start(vec![Canned::ok(&format!(
            r#"{{"account":{ACCOUNT},"session":"tok","recovery_code":"RCV-2"}}"#
        ))]);

        HubClient::new(&hub.url)
            .recover("Alex", "RCV-1", "new password")
            .unwrap();

        let body: Value = serde_json::from_str(&hub.requests()[0].body).unwrap();
        assert_eq!(
            body,
            json!({"username": "Alex", "recovery_code": "RCV-1", "new_password": "new password"})
        );
    }

    const REALM: &str = r#"{"id":"00000000-0000-0000-0000-00000000000a","name":"Home","owner":{"id":"00000000-0000-0000-0000-000000000001","username":"Alex"},"state":"running","address":{"host":"play.example","port":25000},"members":[{"id":"00000000-0000-0000-0000-000000000002","username":"Steve","role":"operator"}]}"#;

    #[test]
    fn the_realms_are_listed_and_created_with_the_session() {
        let hub = TestHub::start(vec![
            Canned::ok(&format!("[{REALM}]")),
            Canned {
                status: 201,
                body: REALM.into(),
            },
        ]);
        let client = HubClient::new(&hub.url);

        let realms = client.realms("tok").unwrap();
        let request = CreateRealmRequest {
            name: "Home".into(),
            settings: rustcraft_protocol::api::RealmSettings {
                seed: Some(7),
                game_mode: Some("creative".into()),
                ..Default::default()
            },
        };
        let created = client.create_realm("tok", &request).unwrap();

        assert_eq!(realms.len(), 1);
        assert_eq!(realms[0].members[0].role, MemberRole::Operator);
        assert_eq!(realms[0].address.as_ref().unwrap().port, 25000);
        assert_eq!(created.name, "Home");
        let requests = hub.requests();
        assert_eq!(
            (requests[0].method.as_str(), requests[0].path.as_str()),
            ("GET", "/v1/realms")
        );
        assert_eq!(
            (requests[1].method.as_str(), requests[1].path.as_str()),
            ("POST", "/v1/realms")
        );
        for request in &requests {
            assert_eq!(request.header("authorization"), Some("Bearer tok"));
        }
        let body: Value = serde_json::from_str(&requests[1].body).unwrap();
        assert_eq!(
            body,
            json!({"name": "Home", "settings": {"seed": 7, "game_mode": "creative"}})
        );
    }

    #[test]
    fn a_realm_is_started_stopped_and_deleted_by_its_id() {
        let id = Uuid::from_u128(10);
        let hub = TestHub::start(vec![
            Canned::ok(r#"{"state":"starting","host":"play.example","port":25002}"#),
            Canned {
                status: 202,
                body: String::new(),
            },
            Canned {
                status: 204,
                body: String::new(),
            },
        ]);
        let client = HubClient::new(&hub.url);

        let started = client.start_realm("tok", id).unwrap();
        client.stop_realm("tok", id).unwrap();
        client.delete_realm("tok", id).unwrap();

        assert_eq!(
            (started.host.as_str(), started.port),
            ("play.example", 25002)
        );
        let calls: Vec<(String, String)> = hub
            .requests()
            .into_iter()
            .map(|request| (request.method, request.path))
            .collect();
        let path = format!("/v1/realms/{id}");
        assert_eq!(
            calls,
            [
                ("POST".to_string(), format!("{path}/start")),
                ("POST".to_string(), format!("{path}/stop")),
                ("DELETE".to_string(), path),
            ]
        );
    }

    #[test]
    fn no_free_slot_is_an_api_error_with_its_message() {
        let hub = TestHub::start(vec![Canned::error(
            503,
            "no_free_slot",
            "The hub runs as many worlds as it can. Try again later.",
        )]);
        let error = HubClient::new(&hub.url)
            .start_realm("tok", Uuid::from_u128(10))
            .unwrap_err();
        assert_eq!(
            error,
            HubError::Api {
                code: ErrorCode::NoFreeSlot,
                message: "The hub runs as many worlds as it can. Try again later.".into()
            }
        );
    }

    #[test]
    fn members_are_set_and_removed_by_their_name_encoded() {
        let id = Uuid::from_u128(10);
        let no_content = || Canned {
            status: 204,
            body: String::new(),
        };
        let hub = TestHub::start(vec![no_content(), no_content()]);
        let client = HubClient::new(&hub.url);

        client
            .set_member("tok", id, "Steve", MemberRole::Operator)
            .unwrap();
        client.remove_member("tok", id, "a b/c").unwrap();

        let requests = hub.requests();
        assert_eq!(requests[0].method, "PUT");
        assert_eq!(requests[0].path, format!("/v1/realms/{id}/members/Steve"));
        let body: Value = serde_json::from_str(&requests[0].body).unwrap();
        assert_eq!(body, json!({"role": "operator"}));
        assert_eq!(requests[1].method, "DELETE");
        assert_eq!(
            requests[1].path,
            format!("/v1/realms/{id}/members/a%20b%2Fc")
        );
    }

    #[test]
    fn nothing_listening_is_unreachable() {
        // A port that was free a moment ago
        let port = {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            listener.local_addr().unwrap().port()
        };
        let error = HubClient::new(&format!("http://127.0.0.1:{port}"))
            .me("t")
            .unwrap_err();
        assert!(matches!(error, HubError::Unreachable(_)), "{error:?}");
    }

    #[test]
    fn an_answer_that_is_not_json_is_malformed() {
        let hub = TestHub::start(vec![Canned::ok("not json")]);
        let error = HubClient::new(&hub.url).me("t").unwrap_err();
        assert!(matches!(error, HubError::Malformed(_)), "{error:?}");
    }
}
