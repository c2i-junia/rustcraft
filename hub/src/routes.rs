//! The routes of the hub API.

use axum::{
    extract::{FromRequest, Request, State},
    routing::get,
    Json, Router,
};
use rustcraft_protocol::{
    api::{ErrorCode, HubKey, WellKnown, API_VERSION},
    identity::key_to_hex,
    PROTOCOL_ID,
};
use serde::de::DeserializeOwned;

use crate::{accounts, error::AppError, AppState, SIGNING_KEY_ID};

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/.well-known/rustcraft-hub", get(well_known))
        .route("/healthz", get(healthz))
        .merge(accounts::routes())
        .with_state(state)
}

async fn well_known(State(state): State<AppState>) -> Json<WellKnown> {
    Json(WellKnown {
        name: state.name.to_string(),
        url: state.public_url.to_string(),
        api_version: API_VERSION,
        keys: vec![HubKey {
            id: SIGNING_KEY_ID,
            public_key: key_to_hex(&state.signing_key.verifying_key().to_bytes()),
        }],
        protocol_id: PROTOCOL_ID,
    })
}

/// 200 when the database answers, 500 otherwise.
async fn healthz(State(state): State<AppState>) -> Result<&'static str, AppError> {
    sqlx::query("SELECT 1").execute(&state.db).await?;
    Ok("ok")
}

/// A JSON body whose rejection is an `invalid_request` error of the spec.
pub struct ApiJson<T>(pub T);

impl<T: DeserializeOwned, S: Send + Sync> FromRequest<S> for ApiJson<T> {
    type Rejection = AppError;

    async fn from_request(request: Request, state: &S) -> Result<Self, AppError> {
        match Json::<T>::from_request(request, state).await {
            Ok(Json(value)) => Ok(Self(value)),
            Err(rejection) => Err(AppError::new(
                ErrorCode::InvalidRequest,
                rejection.body_text(),
            )),
        }
    }
}
