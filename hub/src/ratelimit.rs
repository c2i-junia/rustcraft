//! The client's address and the limits on sign-ups and failed logins.

use std::{
    collections::{HashMap, VecDeque},
    hash::Hash,
    net::{IpAddr, Ipv6Addr, SocketAddr},
    sync::Mutex,
    time::{Duration, Instant},
};

use axum::{
    extract::{ConnectInfo, FromRequestParts},
    http::{request::Parts, HeaderMap},
};
use hmac::{Hmac, Mac};
use rustcraft_protocol::api::ErrorCode;
use sha2::Sha256;
use sqlx::PgConnection;
use time::OffsetDateTime;

use crate::error::AppError;

/// The address of the client: the TCP peer, or the last address of `X-Forwarded-For` when the
/// peer is the local reverse proxy.
pub struct ClientIp(pub IpAddr);

impl<S: Sync> FromRequestParts<S> for ClientIp {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, AppError> {
        let Some(ConnectInfo(peer)) = parts.extensions.get::<ConnectInfo<SocketAddr>>() else {
            tracing::error!("the router is served without its connection info");
            return Err(AppError::new(
                ErrorCode::Internal,
                "Something went wrong on the hub.",
            ));
        };
        Ok(Self(client_ip(peer.ip(), &parts.headers)))
    }
}

pub fn client_ip(peer: IpAddr, headers: &HeaderMap) -> IpAddr {
    let peer = peer.to_canonical();
    if !peer.is_loopback() {
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

/// Refuses a sign-up at `now` beyond the limits, with the seconds to wait.
pub async fn check_signup(
    conn: &mut PgConnection,
    ip_key: &[u8; 32],
    trusted: bool,
    now: OffsetDateTime,
) -> Result<(), AppError> {
    let limits: &[_] = if trusted {
        &SIGNUPS_PER_TRUSTED_ADDRESS
    } else {
        &SIGNUPS_PER_ADDRESS
    };
    for &(window, limit) in limits {
        let nth: Option<OffsetDateTime> = sqlx::query_scalar(
            "SELECT created_at FROM signups WHERE ip_key = $1 AND created_at > $2 \
             ORDER BY created_at DESC OFFSET $3 LIMIT 1",
        )
        .bind(&ip_key[..])
        .bind(now - window)
        .bind(limit - 1)
        .fetch_optional(&mut *conn)
        .await?;
        if let Some(nth) = nth {
            return Err(AppError::new(
                ErrorCode::RateLimited,
                "Too many accounts were created from your address. Try again later.",
            )
            .retry_after(seconds(nth + window - now)));
        }
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
    Ok(())
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

    /// Drops the keys with no event left in the window.
    fn prune(&mut self, now: Instant) {
        let window = self.window;
        self.hits.retain(|_, hits| {
            hits.back()
                .is_some_and(|&t| now.saturating_duration_since(t) < window)
        });
    }
}

/// Failed logins, in memory: 5 per (username, address) and 30 per address, over 15 minutes.
pub struct LoginLimiter {
    per_name: Mutex<SlidingWindow<(String, IpAddr)>>,
    per_address: Mutex<SlidingWindow<IpAddr>>,
}

impl Default for LoginLimiter {
    fn default() -> Self {
        let window = Duration::from_secs(15 * 60);
        Self {
            per_name: Mutex::new(SlidingWindow::new(5, window)),
            per_address: Mutex::new(SlidingWindow::new(30, window)),
        }
    }
}

impl LoginLimiter {
    /// Refuses a login attempt at `now` once either limit is reached.
    pub fn check(&self, username: &str, ip: IpAddr, now: Instant) -> Result<(), AppError> {
        let ip = subnet(ip);
        let wait = self
            .per_name
            .lock()
            .unwrap()
            .retry_after(&(username.to_lowercase(), ip), now)
            .max(self.per_address.lock().unwrap().retry_after(&ip, now));
        match wait {
            Some(wait) => Err(AppError::new(
                ErrorCode::RateLimited,
                "Too many failed logins. Try again later.",
            )
            .retry_after(wait.as_secs_f64().ceil().max(1.0) as u64)),
            None => Ok(()),
        }
    }

    pub fn record_failure(&self, username: &str, ip: IpAddr, now: Instant) {
        let ip = subnet(ip);
        self.per_name
            .lock()
            .unwrap()
            .record((username.to_lowercase(), ip), now);
        self.per_address.lock().unwrap().record(ip, now);
    }

    /// Forgets the keys with no recent failure, so that the maps do not grow forever.
    pub fn prune(&self, now: Instant) {
        self.per_name.lock().unwrap().prune(now);
        self.per_address.lock().unwrap().prune(now);
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
    fn x_forwarded_for_counts_only_from_the_local_proxy() {
        let headers = forwarded(&["10.0.0.1, 203.0.113.7"]);
        assert_eq!(client_ip(ip("127.0.0.1"), &headers), ip("203.0.113.7"));
        assert_eq!(client_ip(ip("::1"), &headers), ip("203.0.113.7"));
        assert_eq!(client_ip(ip("198.51.100.2"), &headers), ip("198.51.100.2"));
        assert_eq!(
            client_ip(ip("127.0.0.1"), &forwarded(&["10.0.0.1", "2001:db8::5"])),
            ip("2001:db8::5")
        );
        assert_eq!(
            client_ip(ip("127.0.0.1"), &HeaderMap::new()),
            ip("127.0.0.1")
        );
        assert_eq!(
            client_ip(ip("::ffff:198.51.100.2"), &headers),
            ip("198.51.100.2")
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
    fn failed_logins_are_limited_per_name_and_per_address() {
        let limiter = LoginLimiter::default();
        let now = Instant::now();
        for _ in 0..5 {
            limiter.check("Alex", ip("203.0.113.1"), now).unwrap();
            limiter.record_failure("Alex", ip("203.0.113.1"), now);
        }
        let error = limiter.check("alex", ip("203.0.113.1"), now).unwrap_err();
        assert_eq!(error.code, ErrorCode::RateLimited);
        assert_eq!(error.retry_after, Some(900));
        limiter.check("Alex", ip("203.0.113.2"), now).unwrap();
        limiter.check("Sam", ip("203.0.113.1"), now).unwrap();
        for i in 0..25 {
            limiter.record_failure(&format!("name{i}"), ip("203.0.113.1"), now);
        }
        assert!(limiter.check("Sam", ip("203.0.113.1"), now).is_err());
        assert!(limiter
            .check("Sam", ip("203.0.113.1"), now + Duration::from_secs(900))
            .is_ok());
    }
}
