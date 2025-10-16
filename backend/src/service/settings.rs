use std::sync::Arc;

use tracing::warn;

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
        baidu_error: snapshot.baidu_error,
        deepseek_error: snapshot.deepseek_error,
        translate_descriptions: snapshot.translate_descriptions,
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

    if let Some(flag) = payload.translate_descriptions {
        let value = if flag { "true" } else { "false" };
        repo::settings::upsert_setting(pool, "translation.translate_descriptions", value).await?;
        update.translate_descriptions = Some(flag);
    }

    if let Err(err) = translator.update_credentials(update) {
        let message = err.to_string();
        if message.contains("unavailable") {
            warn!(
                error = %err,
                "translator provider unavailable when updating credentials"
            );
            let user_message = if message.contains("Baidu") {
                "百度翻译暂不可用，请确认凭据有效并稍后重试"
            } else if message.contains("Deepseek") {
                "Deepseek 翻译暂不可用，请检查 API Key 或稍后重试"
            } else {
                "翻译服务暂不可用，请检查配置"
            };
            return Err(AppError::BadRequest(user_message.into()));
        }
        return Err(AppError::Internal(err));
    }

    if let Some(provider_raw) = payload.provider {
        let provider = provider_raw
            .trim()
            .parse::<TranslatorProvider>()
            .map_err(|_| AppError::BadRequest("不支持的翻译服务".into()))?;
        repo::settings::upsert_setting(pool, "translation.provider", provider.as_str()).await?;
    }

    get_translation_settings(translator).await
}
