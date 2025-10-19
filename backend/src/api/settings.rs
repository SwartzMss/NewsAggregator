use axum::{extract::State, Json};

use crate::{
    app::AppState,
    error::AppResult,
    model::{
        TranslationSettingsOut, TranslationSettingsUpdate, AiDedupSettingsOut, AiDedupSettingsUpdate,
        ModelSettingsOut, ModelSettingsUpdate,
    },
    service,
};

pub async fn get_translation_settings(
    State(state): State<AppState>,
) -> AppResult<Json<TranslationSettingsOut>> {
    let settings = service::settings::get_translation_settings(&state.translator).await?;
    Ok(Json(settings))
}

pub async fn update_translation_settings(
    State(state): State<AppState>,
    Json(payload): Json<TranslationSettingsUpdate>,
) -> AppResult<Json<TranslationSettingsOut>> {
    let settings =
        service::settings::update_translation_settings(&state.pool, &state.translator, payload)
            .await?;
    Ok(Json(settings))
}

pub async fn get_model_settings(
    State(state): State<AppState>,
) -> AppResult<Json<ModelSettingsOut>> {
    let settings = service::settings::get_model_settings(&state.translator).await?;
    Ok(Json(settings))
}

pub async fn update_model_settings(
    State(state): State<AppState>,
    Json(payload): Json<ModelSettingsUpdate>,
) -> AppResult<Json<ModelSettingsOut>> {
    let settings = service::settings::update_model_settings(&state.pool, &state.translator, payload).await?;
    Ok(Json(settings))
}

#[derive(serde::Deserialize)]
pub struct ModelTestPayload { pub provider: String }

pub async fn test_model_connectivity(
    State(state): State<AppState>,
    Json(payload): Json<ModelTestPayload>,
) -> AppResult<Json<serde_json::Value>> {
    service::settings::test_model_connectivity(&state.translator, &payload.provider).await?;
    Ok(Json(serde_json::json!({"ok": true})))
}

pub async fn get_ai_dedup_settings(
    State(state): State<AppState>,
) -> AppResult<Json<AiDedupSettingsOut>> {
    let settings = service::settings::get_ai_dedup_settings(&state.pool, &state.translator).await?;
    Ok(Json(settings))
}

pub async fn update_ai_dedup_settings(
    State(state): State<AppState>,
    Json(payload): Json<AiDedupSettingsUpdate>,
) -> AppResult<Json<AiDedupSettingsOut>> {
    let settings = service::settings::update_ai_dedup_settings(&state.pool, &state.translator, payload).await?;
    Ok(Json(settings))
}
