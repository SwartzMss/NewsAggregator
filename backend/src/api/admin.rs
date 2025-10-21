use axum::{extract::State, Json};

use crate::{app::AppState, auth, error::AppResult, model};
use crate::repo::events::{self as repo_events, NewEvent};

pub async fn login(
    State(state): State<AppState>,
    Json(payload): Json<model::AdminLoginPayload>,
) -> AppResult<Json<model::AdminLoginResponse>> {
    if !state
        .admin
        .verify_credentials(&payload.username, &payload.password)
    {
        return Err(auth::invalid_credentials_error());
    }

    let token = state.admin.issue_session().await;

    // Record a simple admin login event (no source_domain)
    let _ = repo_events::upsert_event(
        &state.pool,
        &NewEvent { level: "info".to_string(), code: "ADMIN_LOGIN".to_string(), source_domain: None },
        0,
    ).await;

    Ok(Json(model::AdminLoginResponse {
        token,
        expires_in: state.admin.ttl_secs(),
    }))
}

pub async fn logout(
    State(state): State<AppState>,
    Json(payload): Json<model::AdminLogoutPayload>,
) -> AppResult<Json<serde_json::Value>> {
    state.admin.revoke_session(&payload.token).await;
    // Record an admin logout event
    let _ = repo_events::upsert_event(
        &state.pool,
        &NewEvent { level: "info".to_string(), code: "ADMIN_LOGOUT".to_string(), source_domain: None },
        0,
    ).await;
    Ok(Json(serde_json::json!({ "ok": true })))
}
