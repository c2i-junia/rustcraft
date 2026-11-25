//! The error every route returns: the spec's body and status codes.

use axum::{
    http::{header::RETRY_AFTER, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use rustcraft_protocol::api::{ErrorBody, ErrorCode};

/// A failed request. It renders as `{"error": "<code>", "message": "<sentence>"}`.
#[derive(Debug)]
pub struct AppError {
    pub code: ErrorCode,
    pub message: String,
    /// Seconds, sent in a `Retry-After` header.
    pub retry_after: Option<u64>,
}

impl AppError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            retry_after: None,
        }
    }

    /// The same error, telling the client to wait that many seconds.
    pub fn retry_after(self, seconds: u64) -> Self {
        Self {
            retry_after: Some(seconds),
            ..self
        }
    }

    /// The HTTP status of an error code.
    pub fn status(&self) -> StatusCode {
        match self.code {
            ErrorCode::InvalidUsername
            | ErrorCode::InvalidPassword
            | ErrorCode::WeakPassword
            | ErrorCode::InvalidRequest => StatusCode::BAD_REQUEST,
            ErrorCode::UsernameTaken => StatusCode::CONFLICT,
            ErrorCode::BadCredentials | ErrorCode::Unauthorized => StatusCode::UNAUTHORIZED,
            ErrorCode::Forbidden | ErrorCode::QuotaReached => StatusCode::FORBIDDEN,
            ErrorCode::NotFound => StatusCode::NOT_FOUND,
            ErrorCode::RateLimited => StatusCode::TOO_MANY_REQUESTS,
            ErrorCode::SignupsClosed | ErrorCode::NoFreeSlot => StatusCode::SERVICE_UNAVAILABLE,
            ErrorCode::Internal | ErrorCode::Unknown => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = self.status();
        let body = ErrorBody {
            error: self.code,
            message: self.message,
        };
        let mut response = (status, Json(body)).into_response();
        if let Some(seconds) = self.retry_after {
            response.headers_mut().insert(RETRY_AFTER, seconds.into());
        }
        response
    }
}

impl From<sqlx::Error> for AppError {
    /// The cause is logged and never sent to the client.
    fn from(error: sqlx::Error) -> Self {
        tracing::error!(%error, "database error");
        Self::new(ErrorCode::Internal, "Something went wrong on the hub.")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http_body_util::BodyExt;

    #[tokio::test]
    async fn an_error_renders_the_spec_body_and_status() {
        let response =
            AppError::new(ErrorCode::UsernameTaken, "That name is taken.").into_response();
        assert_eq!(response.status(), StatusCode::CONFLICT);
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            body,
            serde_json::json!({"error": "username_taken", "message": "That name is taken."})
        );
    }

    #[test]
    fn a_rate_limit_tells_when_to_retry() {
        let response = AppError::new(ErrorCode::RateLimited, "Slow down.")
            .retry_after(42)
            .into_response();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(response.headers()[RETRY_AFTER], "42");
    }

    #[test]
    fn each_code_has_the_status_of_the_spec() {
        let status = |code| AppError::new(code, "").status().as_u16();
        assert_eq!(status(ErrorCode::BadCredentials), 401);
        assert_eq!(status(ErrorCode::Unauthorized), 401);
        assert_eq!(status(ErrorCode::Forbidden), 403);
        assert_eq!(status(ErrorCode::NotFound), 404);
        assert_eq!(status(ErrorCode::RateLimited), 429);
        assert_eq!(status(ErrorCode::SignupsClosed), 503);
        assert_eq!(status(ErrorCode::InvalidRequest), 400);
        assert_eq!(status(ErrorCode::Internal), 500);
    }
}
