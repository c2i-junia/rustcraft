//! The static files of the web admin, from `--admin-dir`, under `/admin/`. A path with no file
//! is client routing and gets `index.html`, but under `_app/`, where the built assets are. No
//! directory is listed, and no path leads out of the directory.

use std::{fs, path::PathBuf, sync::Arc};

use anyhow::Context;
use axum::{
    body::Body,
    extract::{Request, State},
    http::{
        header::{
            CACHE_CONTROL, CONTENT_SECURITY_POLICY, LOCATION, X_CONTENT_TYPE_OPTIONS,
            X_FRAME_OPTIONS,
        },
        HeaderName, HeaderValue, StatusCode, Uri,
    },
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use sha2::{Digest, Sha256};
use tower::ServiceExt;
use tower_http::services::{ServeDir, ServeFile};

const REFERRER_POLICY: HeaderName = HeaderName::from_static("referrer-policy");
const IMMUTABLE: &str = "public, max-age=31536000, immutable";

/// The built interface: its directory, and the `Content-Security-Policy` of its pages, which
/// allows the inline scripts of its `index.html` by their hashes.
#[derive(Debug)]
pub struct AdminUi {
    dir: PathBuf,
    csp: HeaderValue,
}

impl AdminUi {
    /// Reads `index.html` of `dir` for the hashes of its inline scripts.
    pub fn load(dir: PathBuf) -> anyhow::Result<Self> {
        let index = dir.join("index.html");
        let html = fs::read_to_string(&index)
            .with_context(|| format!("reading {} of --admin-dir", index.display()))?;
        let mut scripts = String::from("'self'");
        for hash in inline_script_hashes(&html) {
            scripts.push_str(&format!(" 'sha256-{hash}'"));
        }
        let csp = format!(
            "default-src 'self'; script-src {scripts}; style-src 'self' 'unsafe-inline'; \
             img-src 'self' data:; connect-src 'self'; frame-ancestors 'none'; \
             base-uri 'none'; form-action 'self'; object-src 'none'"
        );
        Ok(Self {
            dir,
            csp: HeaderValue::try_from(csp)?,
        })
    }
}

/// The SHA-256, base64, of the content of each `<script>` without a `src`, as CSP hashes it.
fn inline_script_hashes(html: &str) -> Vec<String> {
    let mut hashes = Vec::new();
    let mut rest = html;
    while let Some(start) = rest.find("<script") {
        let after = &rest[start + "<script".len()..];
        if !after.starts_with(|c: char| c == '>' || c.is_ascii_whitespace()) {
            rest = after;
            continue;
        }
        let Some(open_end) = after.find('>') else {
            break;
        };
        let attributes = &after[..open_end];
        let content = &after[open_end + 1..];
        let Some(close) = content.find("</script") else {
            break;
        };
        let has_src = attributes
            .split_ascii_whitespace()
            .any(|attribute| attribute == "src" || attribute.starts_with("src="));
        if !has_src {
            hashes.push(STANDARD.encode(Sha256::digest(&content[..close])));
        }
        rest = &content[close..];
    }
    hashes
}

pub fn routes(ui: Arc<AdminUi>) -> Router {
    Router::new()
        .route("/admin", get(to_slash))
        .route("/admin/", get(serve))
        .route("/admin/{*path}", get(serve))
        .with_state(ui)
}

/// The pages' headers, and how long the response may be cached.
fn harden(ui: &AdminUi, mut response: Response, cache: &'static str) -> Response {
    let headers = response.headers_mut();
    headers.insert(CONTENT_SECURITY_POLICY, ui.csp.clone());
    headers.insert(X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    headers.insert(REFERRER_POLICY, HeaderValue::from_static("no-referrer"));
    headers.insert(X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    headers.insert(CACHE_CONTROL, HeaderValue::from_static(cache));
    response
}

/// `/admin` is `/admin/`, where the relative paths of the pages resolve.
async fn to_slash(State(ui): State<Arc<AdminUi>>) -> Response {
    let response = (
        StatusCode::PERMANENT_REDIRECT,
        [(LOCATION, HeaderValue::from_static("/admin/"))],
    )
        .into_response();
    harden(&ui, response, "no-cache")
}

async fn serve(State(ui): State<Arc<AdminUi>>, request: Request) -> Response {
    let path = request
        .uri()
        .path()
        .strip_prefix("/admin")
        .unwrap_or("/")
        .to_owned();
    let Ok(uri) = path.parse::<Uri>() else {
        return harden(&ui, StatusCode::NOT_FOUND.into_response(), "no-cache");
    };
    let (mut parts, _) = request.into_parts();
    parts.uri = uri;
    let file = Request::from_parts(parts.clone(), Body::empty());
    let response = ServeDir::new(&ui.dir)
        .append_index_html_on_directories(false)
        .oneshot(file)
        .await
        .map(|response| response.map(Body::new))
        .unwrap_or_else(|never| match never {});
    if path.starts_with("/_app/") {
        let cache = if path.starts_with("/_app/immutable/") && response.status().is_success() {
            IMMUTABLE
        } else {
            "no-cache"
        };
        return harden(&ui, response, cache);
    }
    if response.status() != StatusCode::NOT_FOUND {
        return harden(&ui, response, "no-cache");
    }
    let index = Request::from_parts(parts, Body::empty());
    let response = ServeFile::new(ui.dir.join("index.html"))
        .oneshot(index)
        .await
        .map(|response| response.map(Body::new))
        .unwrap_or_else(|never| match never {});
    harden(&ui, response, "no-cache")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_inline_scripts_are_hashed_and_the_others_not() {
        let html = r#"<head><script type="module" src="./_app/start.js"></script>
            <scripts>no</scripts><script>
            boot()</script><script nonce="x">two()</script></head>"#;
        let hashes = inline_script_hashes(html);
        assert_eq!(
            hashes,
            [
                STANDARD.encode(Sha256::digest("\n            boot()")),
                STANDARD.encode(Sha256::digest("two()")),
            ]
        );
        assert!(inline_script_hashes("<p>none</p>").is_empty());
    }
}
