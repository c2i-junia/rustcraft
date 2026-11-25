//! The rules of usernames and passwords, their Argon2id hashes and the recovery codes.

use std::sync::Arc;

use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Algorithm, Argon2, Params, Version,
};
use rand::{rngs::OsRng, Rng};
use rustcraft_protocol::api::ErrorCode;
use tokio::sync::Semaphore;

use crate::error::AppError;

/// Names nobody may take, compared case-insensitively.
const RESERVED: [&str; 13] = [
    "admin",
    "administrator",
    "rustcraft",
    "hub",
    "server",
    "moderator",
    "mod",
    "support",
    "system",
    "root",
    "owner",
    "staff",
    "official",
];

/// The bounds of a password, in `char`s.
pub const PASSWORD_LENGTH: std::ops::RangeInclusive<usize> = 10..=128;

/// The alphabet of recovery codes: Crockford's base32, without `I L O U`.
const CROCKFORD: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// 3 to 20 characters of `A-Z a-z 0-9 _`, and not a reserved name.
pub fn check_username(username: &str) -> Result<(), AppError> {
    let well_formed = (3..=20).contains(&username.len())
        && username
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_');
    if !well_formed {
        return Err(AppError::new(
            ErrorCode::InvalidUsername,
            "A username has 3 to 20 letters, digits or underscores.",
        ));
    }
    if RESERVED.iter().any(|r| r.eq_ignore_ascii_case(username)) {
        return Err(AppError::new(
            ErrorCode::InvalidUsername,
            "That username is reserved.",
        ));
    }
    Ok(())
}

/// 10 to 128 characters, without the username in it, and not too easy to guess.
pub fn check_password(password: &str, username: &str) -> Result<(), AppError> {
    if !PASSWORD_LENGTH.contains(&password.chars().count()) {
        return Err(AppError::new(
            ErrorCode::InvalidPassword,
            "A password has 10 to 128 characters.",
        ));
    }
    if password.to_lowercase().contains(&username.to_lowercase()) {
        return Err(AppError::new(
            ErrorCode::WeakPassword,
            "The password must not contain the username.",
        ));
    }
    if u8::from(zxcvbn::zxcvbn(password, &[username]).score()) < 2 {
        return Err(AppError::new(
            ErrorCode::WeakPassword,
            "That password is too easy to guess.",
        ));
    }
    Ok(())
}

/// Argon2id, m = 19456 KiB, t = 2, p = 1.
fn argon2() -> Argon2<'static> {
    let params = Params::new(19456, 2, 1, None).expect("valid Argon2 parameters");
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
}

fn hash_now(secret: &str) -> String {
    argon2()
        .hash_password(secret.as_bytes(), &SaltString::generate(&mut OsRng))
        .expect("Argon2 hashes any input")
        .to_string()
}

fn verify_now(secret: &str, hash: &str) -> bool {
    match PasswordHash::new(hash) {
        Ok(hash) => argon2().verify_password(secret.as_bytes(), &hash).is_ok(),
        Err(error) => {
            tracing::error!(%error, "a stored password hash does not parse");
            false
        }
    }
}

/// Hashes and verifies off the async threads, a few at a time: each run takes 19 MiB and a
/// core, and the others wait their turn.
#[derive(Clone)]
pub struct Hasher {
    permits: Arc<Semaphore>,
    /// Compared against when there is no account, so that a missing name takes as long as a
    /// wrong password. Made at start, so that the first unknown name takes no longer.
    dummy_hash: Arc<str>,
}

impl Default for Hasher {
    /// Half the cores, at least one.
    fn default() -> Self {
        let cores = std::thread::available_parallelism().map_or(1, |n| n.get());
        Self::new((cores / 2).max(1))
    }
}

impl Hasher {
    pub fn new(permits: usize) -> Self {
        Self {
            permits: Arc::new(Semaphore::new(permits)),
            dummy_hash: hash_now(&new_recovery_code().body).into(),
        }
    }

    async fn run<T: Send + 'static>(
        &self,
        work: impl FnOnce() -> T + Send + 'static,
    ) -> Result<T, AppError> {
        let permit = self
            .permits
            .clone()
            .acquire_owned()
            .await
            .expect("the semaphore is never closed");
        let result = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            work()
        })
        .await;
        result.map_err(|error| {
            tracing::error!(%error, "a password hash failed");
            AppError::new(ErrorCode::Internal, "Something went wrong on the hub.")
        })
    }

    /// The PHC string of `secret`.
    pub async fn hash(&self, secret: String) -> Result<String, AppError> {
        self.run(move || hash_now(&secret)).await
    }

    /// Whether `secret` matches `hash`. Without a hash it compares against a dummy one, which
    /// takes the same time, and answers `false`.
    pub async fn verify(&self, secret: String, hash: Option<String>) -> Result<bool, AppError> {
        let dummy_hash = self.dummy_hash.clone();
        self.run(move || match hash {
            Some(hash) => verify_now(&secret, &hash),
            None => {
                verify_now(&secret, &dummy_hash);
                false
            }
        })
        .await
    }
}

/// A new recovery code: what to show, and what to hash.
pub struct RecoveryCode {
    /// `RCV-XXXXX-XXXXX-XXXXX`.
    pub display: String,
    /// The 15 characters after `RCV`, as [`normalize_recovery_code`] gives them back.
    pub body: String,
}

pub fn new_recovery_code() -> RecoveryCode {
    let mut rng = OsRng;
    let body: String = (0..15)
        .map(|_| CROCKFORD[rng.gen_range(0..CROCKFORD.len())] as char)
        .collect();
    RecoveryCode {
        display: format!("RCV-{}-{}-{}", &body[..5], &body[5..10], &body[10..]),
        body,
    }
}

/// The 15 characters of a recovery code as a player typed it: any case, with or without `RCV`,
/// spaces and dashes. `None` when it cannot be one.
pub fn normalize_recovery_code(input: &str) -> Option<String> {
    let mut code: String = input
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .collect::<String>()
        .to_ascii_uppercase();
    if code.len() == 18 && code.starts_with("RCV") {
        code.drain(..3);
    }
    (code.len() == 15 && code.bytes().all(|b| CROCKFORD.contains(&b))).then_some(code)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn code(result: Result<(), AppError>) -> Option<ErrorCode> {
        result.err().map(|e| e.code)
    }

    #[test]
    fn usernames_follow_the_rules_of_the_spec() {
        assert_eq!(code(check_username("Alex_99")), None);
        assert_eq!(code(check_username("abc")), None);
        assert_eq!(code(check_username(&"a".repeat(20))), None);
        for bad in ["ab", &"a".repeat(21), "with space", "dé", "a-b", ""] {
            assert_eq!(
                code(check_username(bad)),
                Some(ErrorCode::InvalidUsername),
                "{bad}"
            );
        }
        for reserved in ["admin", "ADMIN", "Rustcraft", "mod", "Official"] {
            assert_eq!(
                code(check_username(reserved)),
                Some(ErrorCode::InvalidUsername),
                "{reserved}"
            );
        }
    }

    #[test]
    fn passwords_follow_the_rules_of_the_spec() {
        assert_eq!(code(check_password("correct horse battery", "Alex")), None);
        assert_eq!(
            code(check_password("short", "Alex")),
            Some(ErrorCode::InvalidPassword)
        );
        assert_eq!(
            code(check_password(&"x".repeat(129), "Alex")),
            Some(ErrorCode::InvalidPassword)
        );
        // 10 characters of 2 bytes each: the length counts characters, not bytes.
        assert_eq!(code(check_password("éàüöçñøåßæ", "Alex")), None);
        assert_eq!(
            code(check_password("my-ALEX-is-great-1984", "alex")),
            Some(ErrorCode::WeakPassword)
        );
        assert_eq!(
            code(check_password("password123", "Alex")),
            Some(ErrorCode::WeakPassword)
        );
        assert_eq!(
            code(check_password("qwertyuiop", "Alex")),
            Some(ErrorCode::WeakPassword)
        );
    }

    #[test]
    fn a_recovery_code_has_the_format_of_the_spec() {
        let code = new_recovery_code();
        assert_eq!(code.display.len(), 21);
        let groups: Vec<&str> = code.display.split('-').collect();
        assert_eq!(groups[0], "RCV");
        assert_eq!(groups.len(), 4);
        for group in &groups[1..] {
            assert_eq!(group.len(), 5);
            assert!(group.bytes().all(|b| CROCKFORD.contains(&b)));
            assert!(!group.contains(['I', 'L', 'O', 'U']));
        }
        assert_eq!(
            normalize_recovery_code(&code.display).as_deref(),
            Some(code.body.as_str())
        );
    }

    #[test]
    fn recovery_codes_read_in_any_case_with_spaces_and_dashes() {
        let body = Some("7K2QFM9XWA4HTPE".to_owned());
        assert_eq!(normalize_recovery_code("RCV-7K2QF-M9XWA-4HTPE"), body);
        assert_eq!(normalize_recovery_code("rcv 7k2qf m9xwa 4htpe"), body);
        assert_eq!(normalize_recovery_code("7k2qf-m9xwa-4htpe"), body);
        assert_eq!(normalize_recovery_code(" RCV7K2QFM9XWA4HTPE "), body);
        assert_eq!(normalize_recovery_code("RCV-7K2QF-M9XWA-4HTP"), None);
        assert_eq!(normalize_recovery_code("RCV-7K2QF-M9XWA-4HTPU"), None);
    }

    #[tokio::test]
    async fn a_hash_verifies_its_secret_only() {
        let hasher = Hasher::new(1);
        let hash = hasher.hash("correct horse".into()).await.unwrap();
        assert!(hash.starts_with("$argon2id$v=19$m=19456,t=2,p=1$"));
        assert!(hasher
            .verify("correct horse".into(), Some(hash.clone()))
            .await
            .unwrap());
        assert!(!hasher
            .verify("wrong horse".into(), Some(hash))
            .await
            .unwrap());
        assert!(!hasher.verify("correct horse".into(), None).await.unwrap());
    }
}
