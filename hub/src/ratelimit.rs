//! The client's address and the limits on sign-ups and failed logins.

use std::{
    collections::{HashMap, VecDeque},
    hash::Hash,
    net::{IpAddr, Ipv6Addr, SocketAddr},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use axum::{
    extract::{ConnectInfo, FromRequestParts},
    http::{request::Parts, HeaderMap},
};
use hmac::{Hmac, Mac};
use ipnet::IpNet;
use rustcraft_protocol::api::ErrorCode;
use sha2::Sha256;
use sqlx::PgConnection;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{error::AppError, AppState};

/// The address of the client: the TCP peer, or the last address of `X-Forwarded-For` when the
/// peer is one of the trusted proxies.
pub struct ClientIp(pub IpAddr);

impl FromRequestParts<AppState> for ClientIp {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, AppError> {
        let Some(ConnectInfo(peer)) = parts.extensions.get::<ConnectInfo<SocketAddr>>() else {
            tracing::error!("the router is served without its connection info");
            return Err(AppError::new(
                ErrorCode::Internal,
                "Something went wrong on the hub.",
            ));
        };
        Ok(Self(client_ip(
            peer.ip(),
            &parts.headers,
            &state.trusted_proxies,
        )))
    }
}

pub fn client_ip(peer: IpAddr, headers: &HeaderMap, trusted_proxies: &[IpNet]) -> IpAddr {
    let peer = peer.to_canonical();
    if !trusted_proxies.iter().any(|range| range.contains(&peer)) {
        return peer;
    }
    headers
        .get_all("x-forwarded-for")
        .iter()
        .next_back()
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.rsplit(',').next())
        .and_then(|last| last.trim().parse::<IpAddr>().ok())
        .map_or(peer, |ip| ip.to_canonical())
}

/// What the limits count: an IPv4 address, or the /64 of an IPv6 address.
pub fn subnet(ip: IpAddr) -> IpAddr {
    match ip.to_canonical() {
        IpAddr::V4(v4) => IpAddr::V4(v4),
        IpAddr::V6(v6) => IpAddr::V6(Ipv6Addr::from_bits(v6.to_bits() & !((1u128 << 64) - 1))),
    }
}

/// The HMAC-SHA256 of the client's [`subnet`], as `signups` stores it.
pub fn ip_key(secret: &[u8], ip: IpAddr) -> [u8; 32] {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret).expect("HMAC takes any key");
    match subnet(ip) {
        IpAddr::V4(v4) => mac.update(&v4.octets()),
        IpAddr::V6(v6) => mac.update(&v6.octets()[..8]),
    }
    mac.finalize().into_bytes().into()
}

/// Sign-ups per address: 10 an hour and 30 a day.
const SIGNUPS_PER_ADDRESS: [(time::Duration, i64); 2] =
    [(time::Duration::HOUR, 10), (time::Duration::DAY, 30)];
/// Sign-ups per address of a trusted range.
const SIGNUPS_PER_TRUSTED_ADDRESS: [(time::Duration, i64); 1] = [(time::Duration::HOUR, 60)];
/// Sign-ups on the whole hub.
const SIGNUPS_PER_HUB: (time::Duration, i64) = (time::Duration::HOUR, 200);

/// Refuses a sign-up at `now` beyond the limits, with the seconds to wait. Otherwise answers how
/// many more sign-ups the address may make.
pub async fn check_signup(
    conn: &mut PgConnection,
    ip_key: &[u8; 32],
    trusted: bool,
    now: OffsetDateTime,
) -> Result<i64, AppError> {
    let limits: &[_] = if trusted {
        &SIGNUPS_PER_TRUSTED_ADDRESS
    } else {
        &SIGNUPS_PER_ADDRESS
    };
    let mut room = i64::MAX;
    for &(window, limit) in limits {
        let count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM signups WHERE ip_key = $1 AND created_at > $2",
        )
        .bind(&ip_key[..])
        .bind(now - window)
        .fetch_one(&mut *conn)
        .await?;
        room = room.min(limit - count);
        if count < limit {
            continue;
        }
        // The window reopens when the limit-th newest sign-up leaves it.
        let nth: OffsetDateTime = sqlx::query_scalar(
            "SELECT created_at FROM signups WHERE ip_key = $1 AND created_at > $2 \
             ORDER BY created_at DESC OFFSET $3 LIMIT 1",
        )
        .bind(&ip_key[..])
        .bind(now - window)
        .bind(limit - 1)
        .fetch_one(&mut *conn)
        .await?;
        return Err(AppError::new(
            ErrorCode::RateLimited,
            "Too many accounts were created from your address. Try again later.",
        )
        .retry_after(seconds(nth + window - now)));
    }
    let (window, limit) = SIGNUPS_PER_HUB;
    let nth: Option<OffsetDateTime> = sqlx::query_scalar(
        "SELECT created_at FROM signups WHERE created_at > $1 \
         ORDER BY created_at DESC OFFSET $2 LIMIT 1",
    )
    .bind(now - window)
    .bind(limit - 1)
    .fetch_optional(&mut *conn)
    .await?;
    if let Some(nth) = nth {
        return Err(AppError::new(
            ErrorCode::SignupsClosed,
            "Sign-ups are closed for a while. Try again later.",
        )
        .retry_after(seconds(nth + window - now)));
    }
    Ok(room)
}

pub async fn record_signup(
    conn: &mut PgConnection,
    ip_key: &[u8; 32],
    now: OffsetDateTime,
) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO signups (ip_key, created_at) VALUES ($1, $2)")
        .bind(&ip_key[..])
        .bind(now)
        .execute(conn)
        .await?;
    Ok(())
}

/// Forgets the sign-ups older than 30 days.
pub async fn purge_signups(
    conn: &mut PgConnection,
    now: OffsetDateTime,
) -> Result<u64, sqlx::Error> {
    let done = sqlx::query("DELETE FROM signups WHERE created_at <= $1")
        .bind(now - time::Duration::days(30))
        .execute(conn)
        .await?;
    Ok(done.rows_affected())
}

/// Whole seconds, rounded up, at least one.
fn seconds(wait: time::Duration) -> u64 {
    (wait.whole_seconds() + i64::from(wait.subsec_nanoseconds() > 0)).max(1) as u64
}

/// Counts events per key over a sliding window.
struct SlidingWindow<K> {
    limit: usize,
    window: Duration,
    hits: HashMap<K, VecDeque<Instant>>,
}

impl<K: Hash + Eq> SlidingWindow<K> {
    fn new(limit: usize, window: Duration) -> Self {
        Self {
            limit,
            window,
            hits: HashMap::new(),
        }
    }

    /// How long to wait when `key` has reached the limit at `now`.
    fn retry_after(&mut self, key: &K, now: Instant) -> Option<Duration> {
        let hits = self.hits.get_mut(key)?;
        while hits
            .front()
            .is_some_and(|&t| now.saturating_duration_since(t) >= self.window)
        {
            hits.pop_front();
        }
        (hits.len() >= self.limit)
            .then(|| (hits[hits.len() - self.limit] + self.window).saturating_duration_since(now))
    }

    fn record(&mut self, key: K, now: Instant) {
        self.hits.entry(key).or_default().push_back(now);
    }

    /// Takes back the event of `key` recorded at `at`.
    fn forget(&mut self, key: &K, at: Instant) {
        if let Some(hits) = self.hits.get_mut(key) {
            if let Some(position) = hits.iter().position(|&t| t == at) {
                hits.remove(position);
            }
        }
    }

    /// Drops the keys with no event left in the window.
    fn prune(&mut self, now: Instant) {
        let window = self.window;
        self.hits.retain(|_, hits| {
            hits.back()
                .is_some_and(|&t| now.saturating_duration_since(t) < window)
        });
    }
}

/// Certificates issued per account: 30 an hour, in memory.
pub struct CertificateLimiter {
    issued: Mutex<SlidingWindow<Uuid>>,
}

impl Default for CertificateLimiter {
    fn default() -> Self {
        Self {
            issued: Mutex::new(SlidingWindow::new(30, Duration::from_secs(60 * 60))),
        }
    }
}

impl CertificateLimiter {
    /// Counts a certificate of `account` at `now`, or refuses it once 30 were issued in the
    /// last hour.
    pub fn issue(&self, account: Uuid, now: Instant) -> Result<(), AppError> {
        let mut issued = self.issued.lock().unwrap();
        if let Some(wait) = issued.retry_after(&account, now) {
            return Err(AppError::new(
                ErrorCode::RateLimited,
                "Too many certificates were issued for this account. Try again later.",
            )
            .retry_after(wait.as_secs_f64().ceil().max(1.0) as u64));
        }
        issued.record(account, now);
        Ok(())
    }

    /// Forgets the accounts with no certificate in the last hour.
    pub fn prune(&self, now: Instant) {
        self.issued.lock().unwrap().prune(now);
    }
}

/// Starts per hosted world: 10 an hour, in memory.
pub struct RealmStartLimiter {
    starts: Mutex<SlidingWindow<Uuid>>,
}

impl Default for RealmStartLimiter {
    fn default() -> Self {
        Self {
            starts: Mutex::new(SlidingWindow::new(10, Duration::from_secs(60 * 60))),
        }
    }
}

impl RealmStartLimiter {
    /// Counts a start of `realm` at `now`, or refuses it once it was started 10 times in the
    /// last hour.
    pub fn start(&self, realm: Uuid, now: Instant) -> Result<(), AppError> {
        let mut starts = self.starts.lock().unwrap();
        if let Some(wait) = starts.retry_after(&realm, now) {
            return Err(AppError::new(
                ErrorCode::RateLimited,
                "This world was started too often. Try again later.",
            )
            .retry_after(wait.as_secs_f64().ceil().max(1.0) as u64));
        }
        starts.record(realm, now);
        Ok(())
    }

    /// Forgets the worlds not started in the last hour.
    pub fn prune(&self, now: Instant) {
        self.starts.lock().unwrap().prune(now);
    }
}

struct Failures {
    per_name: SlidingWindow<(String, IpAddr)>,
    per_address: SlidingWindow<IpAddr>,
}

/// Failed logins, in memory: 5 per (username, address) and 30 per address, over 15 minutes.
/// An attempt counts as failed from the start, so that parallel guesses cannot all pass before
/// the first one fails; a success takes it back.
pub struct LoginLimiter {
    failures: Mutex<Failures>,
}

/// An attempt counted as failed until [`LoginLimiter::forgive`] takes it back.
pub struct LoginAttempt {
    name: String,
    ip: IpAddr,
    at: Instant,
}

impl Default for LoginLimiter {
    fn default() -> Self {
        let window = Duration::from_secs(15 * 60);
        Self {
            failures: Mutex::new(Failures {
                per_name: SlidingWindow::new(5, window),
                per_address: SlidingWindow::new(30, window),
            }),
        }
    }
}

impl LoginLimiter {
    /// Counts an attempt at `now` as failed, or refuses it once either limit is reached.
    pub fn attempt(
        &self,
        username: &str,
        ip: IpAddr,
        now: Instant,
    ) -> Result<LoginAttempt, AppError> {
        let ip = subnet(ip);
        let name = username.to_lowercase();
        let mut failures = self.failures.lock().unwrap();
        let wait = failures
            .per_name
            .retry_after(&(name.clone(), ip), now)
            .max(failures.per_address.retry_after(&ip, now));
        if let Some(wait) = wait {
            return Err(AppError::new(
                ErrorCode::RateLimited,
                "Too many failed logins. Try again later.",
            )
            .retry_after(wait.as_secs_f64().ceil().max(1.0) as u64));
        }
        failures.per_name.record((name.clone(), ip), now);
        failures.per_address.record(ip, now);
        Ok(LoginAttempt { name, ip, at: now })
    }

    /// Takes back a successful attempt.
    pub fn forgive(&self, attempt: LoginAttempt) {
        let mut failures = self.failures.lock().unwrap();
        failures
            .per_name
            .forget(&(attempt.name, attempt.ip), attempt.at);
        failures.per_address.forget(&attempt.ip, attempt.at);
    }

    /// Forgets the keys with no recent failure, so that the maps do not grow forever.
    pub fn prune(&self, now: Instant) {
        let mut failures = self.failures.lock().unwrap();
        failures.per_name.prune(now);
        failures.per_address.prune(now);
    }
}

/// The sign-ups in progress per address. Each takes a slot before its costly hashes, and there
/// are no more slots than sign-ups left to the address, so parallel requests cannot all hash
/// before the limit, checked again under a lock, refuses them.
#[derive(Default)]
pub struct SignupGate {
    in_flight: Mutex<HashMap<[u8; 32], i64>>,
}

/// A sign-up in progress; dropping it frees the slot.
pub struct SignupSlot {
    gate: Arc<SignupGate>,
    ip_key: [u8; 32],
}

impl SignupGate {
    /// Takes a slot for `ip_key`, which has `room` sign-ups left.
    pub fn enter(self: &Arc<Self>, ip_key: [u8; 32], room: i64) -> Result<SignupSlot, AppError> {
        let mut in_flight = self.in_flight.lock().unwrap();
        let count = in_flight.entry(ip_key).or_default();
        if *count >= room {
            if *count == 0 {
                in_flight.remove(&ip_key);
            }
            return Err(AppError::new(
                ErrorCode::RateLimited,
                "Too many accounts are being created from your address. Try again later.",
            )
            .retry_after(1));
        }
        *count += 1;
        Ok(SignupSlot {
            gate: self.clone(),
            ip_key,
        })
    }
}

impl Drop for SignupSlot {
    fn drop(&mut self) {
        let mut in_flight = self.gate.in_flight.lock().unwrap();
        if let Some(count) = in_flight.get_mut(&self.ip_key) {
            *count -= 1;
            if *count <= 0 {
                in_flight.remove(&self.ip_key);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    fn ip(text: &str) -> IpAddr {
        text.parse().unwrap()
    }

    fn forwarded(values: &[&str]) -> HeaderMap {
        let mut headers = HeaderMap::new();
        for value in values {
            headers.append("x-forwarded-for", HeaderValue::from_str(value).unwrap());
        }
        headers
    }

    #[test]
    fn x_forwarded_for_counts_only_from_a_trusted_proxy() {
        let loopback: Vec<IpNet> =
            vec!["127.0.0.1/32".parse().unwrap(), "::1/128".parse().unwrap()];
        let headers = forwarded(&["10.0.0.1, 203.0.113.7"]);
        assert_eq!(
            client_ip(ip("127.0.0.1"), &headers, &loopback),
            ip("203.0.113.7")
        );
        assert_eq!(client_ip(ip("::1"), &headers, &loopback), ip("203.0.113.7"));
        assert_eq!(
            client_ip(ip("198.51.100.2"), &headers, &loopback),
            ip("198.51.100.2")
        );
        assert_eq!(
            client_ip(
                ip("127.0.0.1"),
                &forwarded(&["10.0.0.1", "2001:db8::5"]),
                &loopback
            ),
            ip("2001:db8::5")
        );
        assert_eq!(
            client_ip(ip("127.0.0.1"), &HeaderMap::new(), &loopback),
            ip("127.0.0.1")
        );
        assert_eq!(
            client_ip(ip("::ffff:198.51.100.2"), &headers, &loopback),
            ip("198.51.100.2")
        );
    }

    #[test]
    fn a_proxy_outside_the_loopback_is_trusted_when_listed() {
        let proxies: Vec<IpNet> = vec!["172.30.80.1/32".parse().unwrap()];
        let headers = forwarded(&["203.0.113.7"]);
        assert_eq!(
            client_ip(ip("172.30.80.1"), &headers, &proxies),
            ip("203.0.113.7")
        );
        // Another address of the network, and the loopback, are not the proxy.
        assert_eq!(
            client_ip(ip("172.30.80.2"), &headers, &proxies),
            ip("172.30.80.2")
        );
        assert_eq!(
            client_ip(ip("127.0.0.1"), &headers, &proxies),
            ip("127.0.0.1")
        );
        assert_eq!(
            client_ip(ip("172.30.80.1"), &headers, &[]),
            ip("172.30.80.1")
        );
    }

    #[test]
    fn an_ipv6_address_counts_with_its_64() {
        let secret = [1u8; 32];
        let key = |text| ip_key(&secret, ip(text));
        assert_eq!(key("2001:db8:1:2::1"), key("2001:db8:1:2:ffff::9"));
        assert_ne!(key("2001:db8:1:2::1"), key("2001:db8:1:3::1"));
        assert_ne!(key("203.0.113.1"), key("203.0.113.2"));
        assert_eq!(key("::ffff:203.0.113.1"), key("203.0.113.1"));
        assert_ne!(ip_key(&[2u8; 32], ip("203.0.113.1")), key("203.0.113.1"));
    }

    #[test]
    fn a_sliding_window_refuses_beyond_its_limit_until_the_oldest_event_leaves() {
        let start = Instant::now();
        let at = |s: u64| start + Duration::from_secs(s);
        let mut window = SlidingWindow::new(2, Duration::from_secs(60));
        assert_eq!(window.retry_after(&"a", at(0)), None);
        window.record("a", at(0));
        window.record("a", at(10));
        assert_eq!(
            window.retry_after(&"a", at(20)),
            Some(Duration::from_secs(40))
        );
        assert_eq!(window.retry_after(&"b", at(20)), None);
        assert_eq!(window.retry_after(&"a", at(60)), None);
        window.prune(at(69));
        assert!(window.hits.contains_key("a"));
        window.prune(at(70));
        assert!(window.hits.is_empty());
    }

    #[test]
    fn a_world_is_started_at_most_ten_times_an_hour() {
        let limiter = RealmStartLimiter::default();
        let now = Instant::now();
        let realm = Uuid::from_u128(1);
        for _ in 0..10 {
            limiter.start(realm, now).unwrap();
        }
        let error = limiter.start(realm, now).err().unwrap();
        assert_eq!(error.code, ErrorCode::RateLimited);
        assert_eq!(error.retry_after, Some(3600));
        assert!(limiter.start(Uuid::from_u128(2), now).is_ok());
        assert!(limiter
            .start(realm, now + Duration::from_secs(3600))
            .is_ok());
    }

    #[test]
    fn failed_logins_are_limited_per_name_and_per_address() {
        let limiter = LoginLimiter::default();
        let now = Instant::now();
        let here = ip("203.0.113.1");
        for _ in 0..10 {
            limiter.forgive(limiter.attempt("Alex", here, now).unwrap());
        }
        // Attempts count from the start: these five are in progress, or failed.
        for _ in 0..5 {
            limiter.attempt("Alex", here, now).unwrap();
        }
        let error = limiter.attempt("alex", here, now).err().unwrap();
        assert_eq!(error.code, ErrorCode::RateLimited);
        assert_eq!(error.retry_after, Some(900));
        limiter.forgive(limiter.attempt("Alex", ip("203.0.113.2"), now).unwrap());
        for i in 0..25 {
            limiter.attempt(&format!("name{i}"), here, now).unwrap();
        }
        assert!(limiter.attempt("Sam", here, now).is_err());
        assert!(limiter
            .attempt("Sam", here, now + Duration::from_secs(900))
            .is_ok());
    }

    #[test]
    fn an_address_has_no_more_sign_ups_in_progress_than_it_has_left() {
        let gate = Arc::new(SignupGate::default());
        let first = gate.enter([1; 32], 2).unwrap();
        let second = gate.enter([1; 32], 2).unwrap();
        let error = gate.enter([1; 32], 2).err().unwrap();
        assert_eq!(error.code, ErrorCode::RateLimited);
        assert!(gate.enter([2; 32], 2).is_ok());
        assert!(gate.enter([3; 32], 0).is_err());
        drop(first);
        let third = gate.enter([1; 32], 2).unwrap();
        drop(third);
        drop(second);
        assert!(gate.in_flight.lock().unwrap().is_empty());
    }
}
