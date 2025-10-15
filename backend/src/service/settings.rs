use std::sync::Arc;

use crate::{
    error::{AppError, AppResult},
    model::{TranslationSettingsOut, TranslationSettingsUpdate},
    repo,
    util::translator::{TranslationEngine, TranslatorCredentialsUpdate, TranslatorProvider},
};

pub async fn get_translation_settings(
    translator: &Arc<TranslationEngine>,
) -> AppResult<TranslationSettingsOut> {
    let snapshot = translator.snapshot();
    Ok(TranslationSettingsOut {
        provider: snapshot.provider.as_str().to_string(),
        available_providers: translator
            .available_providers()
            .into_iter()
            .map(|provider| provider.as_str().to_string())
            .collect(),
        baidu_configured: snapshot.baidu_configured,
        deepseek_configured: snapshot.deepseek_configured,
        baidu_app_id_masked: snapshot.baidu_app_id_masked,
        baidu_secret_key_masked: snapshot.baidu_secret_key_masked,
        deepseek_api_key_masked: snapshot.deepseek_api_key_masked,
    })
}

pub async fn update_translation_settings(
    pool: &sqlx::PgPool,
    translator: &Arc<TranslationEngine>,
    payload: TranslationSettingsUpdate,
) -> AppResult<TranslationSettingsOut> {
    let mut update = TranslatorCredentialsUpdate::default();

    if let Some(ref provider_raw) = payload.provider {
        let provider_trimmed = provider_raw.trim();
        if provider_trimmed.is_empty() {
            return Err(AppError::BadRequest("翻译服务类型不能为空".into()));
        }
        let provider = provider_trimmed
            .parse::<TranslatorProvider>()
            .map_err(|_| AppError::BadRequest("不支持的翻译服务".into()))?;
        update.provider = Some(provider);
    }

    if let Some(app_id) = payload.baidu_app_id {
        if app_id.trim().is_empty() {
            repo::settings::delete_setting(pool, "translation.baidu_app_id").await?;
            update.baidu_app_id = Some(String::new());
        } else {
            repo::settings::upsert_setting(pool, "translation.baidu_app_id", &app_id).await?;
            update.baidu_app_id = Some(app_id);
        }
    }

    if let Some(secret) = payload.baidu_secret_key {
        if secret.trim().is_empty() {
            repo::settings::delete_setting(pool, "translation.baidu_secret_key").await?;
            update.baidu_secret_key = Some(String::new());
        } else {
            repo::settings::upsert_setting(pool, "translation.baidu_secret_key", &secret).await?;
            update.baidu_secret_key = Some(secret);
        }
    }

    if let Some(api_key) = payload.deepseek_api_key {
        if api_key.trim().is_empty() {
            repo::settings::delete_setting(pool, "translation.deepseek_api_key").await?;
            update.deepseek_api_key = Some(String::new());
        } else {
            repo::settings::upsert_setting(pool, "translation.deepseek_api_key", &api_key).await?;
            update.deepseek_api_key = Some(api_key);
        }
    }

    translator
        .update_credentials(update)
        .map_err(AppError::Internal)?;

    if let Some(provider_raw) = payload.provider {
        let provider = provider_raw
            .trim()
            .parse::<TranslatorProvider>()
            .map_err(|_| AppError::BadRequest("不支持的翻译服务".into()))?;
        repo::settings::upsert_setting(pool, "translation.provider", provider.as_str()).await?;
    }

    get_translation_settings(translator).await
}
