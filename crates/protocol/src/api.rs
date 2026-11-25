//! The JSON types of the hub API. Keys are hex, certificate bytes and signatures are base64
//! (standard, padded) in the JSON; the helpers of this module convert them.

use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::identity::SignedCertificate;

/// The version of the API the well-known document announces.
pub const API_VERSION: u32 = 1;

/// The machine-readable reason of an error, serialized in snake_case.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    InvalidUsername,
    UsernameTaken,
    InvalidPassword,
    WeakPassword,
    BadCredentials,
    RateLimited,
    SignupsClosed,
    Unauthorized,
    Forbidden,
    NotFound,
    QuotaReached,
    NoFreeSlot,
    InvalidRequest,
    Internal,
}

/// The body of every error response.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorBody {
    pub error: ErrorCode,
    /// An English sentence for people.
    pub message: String,
}

/// An account role granted by the admin.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Admin,
    Host,
}

/// `GET /.well-known/rustcraft-hub`
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WellKnown {
    pub name: String,
    pub url: String,
    pub api_version: u32,
    pub keys: Vec<HubKey>,
    pub protocol_id: u64,
}

/// A public key the hub accepts for certificates.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HubKey {
    pub id: u8,
    /// Hex, 32 bytes.
    pub public_key: String,
}

/// `POST /v1/accounts`
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignUpRequest {
    pub username: String,
    pub password: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_name: Option<String>,
}

/// `POST /v1/sessions`
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_name: Option<String>,
}

/// An account as the sign-up, login and recovery answers carry it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountSummary {
    pub id: Uuid,
    pub username: String,
}

/// The answer to `POST /v1/sessions`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionResponse {
    pub account: AccountSummary,
    /// The bearer token, base64url without padding.
    pub session: String,
}

/// The answer to `POST /v1/accounts` and `POST /v1/recovery`: a session and the new recovery
/// code, shown once.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionWithRecovery {
    pub account: AccountSummary,
    pub session: String,
    pub recovery_code: String,
}

/// `POST /v1/recovery`
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryRequest {
    pub username: String,
    pub recovery_code: String,
    pub new_password: String,
}

/// `GET /v1/me`
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Me {
    pub id: Uuid,
    pub username: String,
    pub roles: Vec<Role>,
    pub realm_quota: u32,
    /// Unix seconds.
    pub created_at: u64,
}

/// `PUT /v1/me/password`
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

/// `DELETE /v1/me`
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeleteAccountRequest {
    pub password: String,
}

/// `POST /v1/certificates`
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CertificateRequest {
    /// Hex, 32 bytes.
    pub device_key: String,
}

/// The answer to `POST /v1/certificates`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CertificateResponse {
    /// Base64 of the certificate bytes.
    pub certificate: String,
    /// Base64 of the 64-byte signature.
    pub signature: String,
    /// Unix seconds.
    pub expires_at: u64,
}

impl CertificateResponse {
    pub fn new(signed: &SignedCertificate, expires_at: u64) -> Self {
        Self {
            certificate: STANDARD.encode(&signed.bytes),
            signature: STANDARD.encode(signed.signature),
            expires_at,
        }
    }

    /// `None` when either field is not base64 or the signature is not 64 bytes.
    pub fn signed_certificate(&self) -> Option<SignedCertificate> {
        Some(SignedCertificate {
            bytes: STANDARD.decode(&self.certificate).ok()?,
            signature: STANDARD.decode(&self.signature).ok()?.try_into().ok()?,
        })
    }
}

/// `GET /v1/revoked`
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Revoked {
    pub ids: Vec<Uuid>,
}

/// `GET /v1/profiles?name=`
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
    pub id: Uuid,
    pub username: String,
}

/// The lifecycle of a hosted world.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RealmState {
    Stopped,
    Starting,
    Running,
    Stopping,
    Failed,
    Archived,
}

/// What a member may do in a hosted world; the owner is not a member role.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemberRole {
    Member,
    Operator,
}

/// The settings of a new world, all optional: the server's defaults apply to what is left out.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealmSettings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,
    /// As the server's `--world-type` takes it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub world_type: Option<String>,
    /// As the server's `--game-mode` takes it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub game_mode: Option<String>,
    /// As the server's `--difficulty` takes it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub difficulty: Option<String>,
}

/// `POST /v1/realms`
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateRealmRequest {
    pub name: String,
    #[serde(default)]
    pub settings: RealmSettings,
}

/// A member of a hosted world, as the owner and the members see it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealmMember {
    pub id: Uuid,
    pub username: String,
    pub role: MemberRole,
}

/// Where to join a running world.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealmAddress {
    pub host: String,
    pub port: u16,
}

/// A hosted world, as `GET /v1/realms` and `POST /v1/realms` list it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Realm {
    pub id: Uuid,
    pub name: String,
    pub owner: AccountSummary,
    pub state: RealmState,
    /// Present while the world is running.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub address: Option<RealmAddress>,
    pub members: Vec<RealmMember>,
}

/// The answer to `POST /v1/realms/{id}/start`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartRealmResponse {
    pub state: RealmState,
    pub host: String,
    pub port: u16,
}

/// `PUT /v1/realms/{id}/members/{username}`
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetMemberRequest {
    pub role: MemberRole,
}

/// The answer to `GET /v1/realms/{id}/config`, read by the world's server.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealmConfig {
    pub members: Vec<RealmMember>,
    pub owner: AccountSummary,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::de::DeserializeOwned;
    use serde_json::{json, Value};

    fn round_trip<T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug>(
        value: &T,
    ) -> Value {
        let json = serde_json::to_value(value).unwrap();
        assert_eq!(&serde_json::from_value::<T>(json.clone()).unwrap(), value);
        json
    }

    fn account() -> AccountSummary {
        AccountSummary {
            id: Uuid::nil(),
            username: "Alex".into(),
        }
    }

    #[test]
    fn error_codes_are_snake_case_in_the_error_body() {
        let body = ErrorBody {
            error: ErrorCode::NoFreeSlot,
            message: "All slots are taken.".into(),
        };
        assert_eq!(
            round_trip(&body),
            json!({"error": "no_free_slot", "message": "All slots are taken."})
        );
        assert_eq!(
            serde_json::to_value(ErrorCode::SignupsClosed).unwrap(),
            "signups_closed"
        );
    }

    #[test]
    fn a_sign_up_request_leaves_out_the_device_name_when_there_is_none() {
        let request = SignUpRequest {
            username: "Alex".into(),
            password: "secret".into(),
            device_name: None,
        };
        assert_eq!(
            round_trip(&request),
            json!({"username": "Alex", "password": "secret"})
        );
        let parsed: SignUpRequest =
            serde_json::from_value(json!({"username": "a", "password": "b", "device_name": "PC"}))
                .unwrap();
        assert_eq!(parsed.device_name.as_deref(), Some("PC"));
    }

    #[test]
    fn session_answers_have_the_fields_of_the_spec() {
        let json = round_trip(&SessionWithRecovery {
            account: account(),
            session: "tok".into(),
            recovery_code: "RCV-7K2QF-M9XWA-4HTPE".into(),
        });
        assert_eq!(json["account"]["username"], "Alex");
        assert_eq!(json["session"], "tok");
        assert_eq!(json["recovery_code"], "RCV-7K2QF-M9XWA-4HTPE");
        let json = round_trip(&SessionResponse {
            account: account(),
            session: "tok".into(),
        });
        assert!(json.get("recovery_code").is_none());
    }

    #[test]
    fn me_lists_roles_in_snake_case() {
        let json = round_trip(&Me {
            id: Uuid::nil(),
            username: "Alex".into(),
            roles: vec![Role::Admin, Role::Host],
            realm_quota: 2,
            created_at: 5,
        });
        assert_eq!(json["roles"], json!(["admin", "host"]));
        assert_eq!(json["realm_quota"], 2);
        assert_eq!(json["created_at"], 5);
    }

    #[test]
    fn request_bodies_have_the_fields_of_the_spec() {
        let json = round_trip(&RecoveryRequest {
            username: "a".into(),
            recovery_code: "c".into(),
            new_password: "p".into(),
        });
        assert_eq!(
            json,
            json!({"username": "a", "recovery_code": "c", "new_password": "p"})
        );
        let json = round_trip(&ChangePasswordRequest {
            current_password: "a".into(),
            new_password: "b".into(),
        });
        assert_eq!(json, json!({"current_password": "a", "new_password": "b"}));
        assert_eq!(
            round_trip(&DeleteAccountRequest {
                password: "a".into()
            }),
            json!({"password": "a"})
        );
        assert_eq!(
            round_trip(&CertificateRequest {
                device_key: "ab".into()
            }),
            json!({"device_key": "ab"})
        );
    }

    #[test]
    fn a_certificate_response_carries_base64_that_decodes_back() {
        let signed = SignedCertificate {
            bytes: vec![1, 2, 3, 4],
            signature: [9; 64],
        };
        let response = CertificateResponse::new(&signed, 42);
        let json = round_trip(&response);
        assert_eq!(json["certificate"], "AQIDBA==");
        assert_eq!(json["expires_at"], 42);
        assert_eq!(response.signed_certificate(), Some(signed));
        let broken = CertificateResponse {
            signature: "AQID".into(),
            ..response
        };
        assert_eq!(broken.signed_certificate(), None);
    }

    #[test]
    fn well_known_has_the_fields_of_the_spec() {
        let json = round_trip(&WellKnown {
            name: "Rustcraft".into(),
            url: "https://hub.rustcraft.dev".into(),
            api_version: API_VERSION,
            keys: vec![HubKey {
                id: 1,
                public_key: "ab".into(),
            }],
            protocol_id: 44,
        });
        assert_eq!(json["api_version"], 1);
        assert_eq!(json["keys"][0], json!({"id": 1, "public_key": "ab"}));
        assert_eq!(json["protocol_id"], 44);
    }

    #[test]
    fn revoked_and_profiles_have_the_fields_of_the_spec() {
        let id = Uuid::from_u128(7);
        assert_eq!(
            round_trip(&Revoked { ids: vec![id] }),
            json!({"ids": [id.to_string()]})
        );
        assert_eq!(
            round_trip(&Profile {
                id,
                username: "Alex".into()
            }),
            json!({"id": id.to_string(), "username": "Alex"})
        );
    }

    #[test]
    fn realms_serialize_states_and_roles_in_snake_case() {
        let realm = Realm {
            id: Uuid::nil(),
            name: "Home".into(),
            owner: account(),
            state: RealmState::Starting,
            address: None,
            members: vec![RealmMember {
                id: Uuid::nil(),
                username: "Bo".into(),
                role: MemberRole::Operator,
            }],
        };
        let json = round_trip(&realm);
        assert_eq!(json["state"], "starting");
        assert!(json.get("address").is_none());
        assert_eq!(json["members"][0]["role"], "operator");
        let started = round_trip(&StartRealmResponse {
            state: RealmState::Running,
            host: "play.rustcraft.dev".into(),
            port: 25000,
        });
        assert_eq!(
            started,
            json!({"state": "running", "host": "play.rustcraft.dev", "port": 25000})
        );
        assert_eq!(
            round_trip(&SetMemberRequest {
                role: MemberRole::Member
            }),
            json!({"role": "member"})
        );
    }

    #[test]
    fn a_create_realm_request_may_leave_the_settings_out() {
        let request: CreateRealmRequest = serde_json::from_value(json!({"name": "Home"})).unwrap();
        assert_eq!(request.settings, RealmSettings::default());
        let json = round_trip(&CreateRealmRequest {
            name: "Home".into(),
            settings: RealmSettings {
                seed: Some(5),
                ..Default::default()
            },
        });
        assert_eq!(json["settings"], json!({"seed": 5}));
    }

    #[test]
    fn a_realm_config_has_members_and_owner() {
        let json = round_trip(&RealmConfig {
            members: Vec::new(),
            owner: account(),
        });
        assert_eq!(json["members"], json!([]));
        assert_eq!(json["owner"]["username"], "Alex");
    }
}
