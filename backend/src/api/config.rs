use axum::{extract::State, Json};

use crate::{app::AppState, config::FrontendPublicConfig};

pub async fn frontend_config(State(state): State<AppState>) -> Json<FrontendPublicConfig> {
    Json(state.config.clone())
}
