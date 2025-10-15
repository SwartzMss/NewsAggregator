use axum::{extract::State, Json};

use crate::{app::AppState, auth, error::AppResult, model};

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
    Ok(Json(serde_json::json!({ "ok": true })))
}
