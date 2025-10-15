use axum::{extract::State, Json};

use crate::{
    app::AppState,
    error::AppResult,
    model::{TranslationSettingsOut, TranslationSettingsUpdate},
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
