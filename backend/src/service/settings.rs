use std::sync::Arc;

use tracing::warn;

use crate::{
    error::{AppError, AppResult},
    model::{
        TranslationSettingsOut, TranslationSettingsUpdate, AiDedupSettingsOut, AiDedupSettingsUpdate,
        ModelSettingsOut, ModelSettingsUpdate,
    },
    repo,
    util::translator::{TranslationEngine, TranslatorCredentialsUpdate, TranslatorProvider},
    ops::events::{self as ops_events, EmitEvent, EventsHub},
};

pub async fn get_translation_settings(
    translator: &Arc<TranslationEngine>,
) -> AppResult<TranslationSettingsOut> {
    let snapshot = translator.snapshot();
    Ok(TranslationSettingsOut {
        // 后台仅允许 Ollama 作为默认服务
        provider: "ollama".to_string(),
        translation_enabled: snapshot.translation_enabled,
        deepseek_configured: snapshot.deepseek_configured,
        ollama_configured: snapshot.ollama_configured,
        deepseek_api_key_masked: snapshot.deepseek_api_key_masked,
        deepseek_error: snapshot.deepseek_error,
        ollama_error: snapshot.ollama_error,
        ollama_base_url: snapshot.ollama_base_url,
        ollama_model: snapshot.ollama_model,
    })
}

pub async fn update_translation_settings(
    pool: &sqlx::PgPool,
    translator: &Arc<TranslationEngine>,
    events: &EventsHub,
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

    // Baidu support removed

    if let Some(api_key) = payload.deepseek_api_key {
        if api_key.trim().is_empty() {
            repo::settings::delete_setting(pool, "translation.deepseek_api_key").await?;
            update.deepseek_api_key = Some(String::new());
        } else {
            repo::settings::upsert_setting(pool, "translation.deepseek_api_key", &api_key).await?;
            update.deepseek_api_key = Some(api_key);
        }
    }

    if let Some(base_url) = payload.ollama_base_url {
        let trimmed = base_url.trim();
        if trimmed.is_empty() {
            repo::settings::delete_setting(pool, "translation.ollama_base_url").await?;
            update.ollama_base_url = Some(String::new());
        } else {
            repo::settings::upsert_setting(pool, "translation.ollama_base_url", trimmed).await?;
            update.ollama_base_url = Some(trimmed.to_string());
        }
    }

    if let Some(model) = payload.ollama_model {
        let trimmed = model.trim();
        if trimmed.is_empty() {
            repo::settings::delete_setting(pool, "translation.ollama_model").await?;
            update.ollama_model = Some(String::new());
        } else {
            repo::settings::upsert_setting(pool, "translation.ollama_model", trimmed).await?;
            update.ollama_model = Some(trimmed.to_string());
        }
    }

    if let Some(flag) = payload.translation_enabled {
        let value = if flag { "true" } else { "false" };
        repo::settings::upsert_setting(pool, "translation.enabled", value).await?;
        update.translation_enabled = Some(flag);
    }

    if let Err(err) = translator.update_credentials(update) {
        let message = err.to_string();
        if message.contains("unavailable") {
            warn!(
                error = %err,
                "translator provider unavailable when updating credentials"
            );
            // emit alert event (non-blocking best-effort)
            let provider = payload.provider.as_deref().unwrap_or("").to_string();
            // event suppressed per new minimal set
            let user_message = if message.contains("Deepseek") {
                "Deepseek 翻译暂不可用，请检查 API Key 或稍后重试"
            } else if message.contains("Ollama") {
                "Ollama 翻译暂不可用，请确认服务地址与模型名称"
            } else {
                "翻译服务暂不可用，请检查配置"
            };
            return Err(AppError::BadRequest(user_message.into()));
        }
        return Err(AppError::Internal(err));
    }

    // 强制仅允许 Ollama 作为默认 provider
    if payload.provider.is_some() || payload.translation_enabled == Some(true) {
        repo::settings::upsert_setting(pool, "translation.provider", "ollama").await?;
    }

    get_translation_settings(translator).await
}

pub async fn get_model_settings(translator: &Arc<TranslationEngine>) -> AppResult<ModelSettingsOut> {
    let snapshot = translator.snapshot();
    Ok(ModelSettingsOut {
        deepseek_api_key_masked: snapshot.deepseek_api_key_masked,
        ollama_base_url: snapshot.ollama_base_url,
        ollama_model: snapshot.ollama_model,
    })
}

pub async fn update_model_settings(
    pool: &sqlx::PgPool,
    translator: &Arc<TranslationEngine>,
    payload: ModelSettingsUpdate,
) -> AppResult<ModelSettingsOut> {
    let mut update = TranslatorCredentialsUpdate::default();
    if let Some(api_key) = payload.deepseek_api_key {
        if api_key.trim().is_empty() {
            repo::settings::delete_setting(pool, "translation.deepseek_api_key").await?;
            update.deepseek_api_key = Some(String::new());
        } else {
            repo::settings::upsert_setting(pool, "translation.deepseek_api_key", &api_key).await?;
            update.deepseek_api_key = Some(api_key);
        }
    }
    if let Some(base_url) = payload.ollama_base_url {
        let trimmed = base_url.trim();
        if trimmed.is_empty() {
            repo::settings::delete_setting(pool, "translation.ollama_base_url").await?;
            update.ollama_base_url = Some(String::new());
        } else {
            repo::settings::upsert_setting(pool, "translation.ollama_base_url", trimmed).await?;
            update.ollama_base_url = Some(trimmed.to_string());
        }
    }
    if let Some(model) = payload.ollama_model {
        let trimmed = model.trim();
        if trimmed.is_empty() {
            repo::settings::delete_setting(pool, "translation.ollama_model").await?;
            update.ollama_model = Some(String::new());
        } else {
            repo::settings::upsert_setting(pool, "translation.ollama_model", trimmed).await?;
            update.ollama_model = Some(trimmed.to_string());
        }
    }

    translator
        .update_credentials(update)
        .map_err(|e| AppError::BadRequest(e.to_string()))?;
    get_model_settings(translator).await
}

pub async fn test_model_connectivity(
    translator: &Arc<TranslationEngine>,
    provider: &str,
) -> AppResult<()> {
    let p = provider
        .trim()
        .parse::<TranslatorProvider>()
        .map_err(|_| AppError::BadRequest("不支持的 provider".into()))?;
    translator
        .test_connectivity(p)
        .await
        .map_err(AppError::Internal)?;
    Ok(())
}

pub async fn get_ai_dedup_settings(
    pool: &sqlx::PgPool,
    translator: &Arc<TranslationEngine>,
) -> AppResult<AiDedupSettingsOut> {
    let enabled_raw = repo::settings::get_setting(pool, "ai_dedup.enabled").await?;
    let provider_raw = repo::settings::get_setting(pool, "ai_dedup.provider").await?;
    let enabled = matches!(enabled_raw.as_deref(), Some("true"));
    let provider = if enabled { provider_raw } else { None };
    let snapshot = translator.snapshot();
    Ok(AiDedupSettingsOut {
        enabled,
        provider,
        deepseek_configured: snapshot.deepseek_configured,
        ollama_configured: snapshot.ollama_configured,
        threshold: 0.6,
        max_checks: 3,
    })
}

pub async fn update_ai_dedup_settings(
    pool: &sqlx::PgPool,
    translator: &Arc<TranslationEngine>, // translator only for configured status
    payload: AiDedupSettingsUpdate,
) -> AppResult<AiDedupSettingsOut> {
    // enabled update
    if let Some(flag) = payload.enabled {
        let value = if flag { "true" } else { "false" };
        repo::settings::upsert_setting(pool, "ai_dedup.enabled", value).await?;
        if !flag {
            // keep provider stored but not returned; do not delete
        }
    }

    // provider update（可选）
    if let Some(provider) = payload.provider.as_ref() {
        let trimmed = provider.trim();
        if trimmed.is_empty() {
            return Err(AppError::BadRequest("AI 去重 provider 不能为空".into()));
        }
        if trimmed != "deepseek" && trimmed != "ollama" {
            return Err(AppError::BadRequest("不支持的 AI 去重 provider".into()));
        }
        repo::settings::upsert_setting(pool, "ai_dedup.provider", trimmed).await?;
    }

    // 若启用但未指定 provider，则按 Deepseek > Ollama 的优先级自动选择；均未配置则报错并引导前往大模型配置
    let enabled_raw = repo::settings::get_setting(pool, "ai_dedup.enabled").await?;
    let provider_raw = repo::settings::get_setting(pool, "ai_dedup.provider").await?;
    if matches!(enabled_raw.as_deref(), Some("true")) && provider_raw.as_deref().map(str::is_empty).unwrap_or(true) {
        let snapshot = translator.snapshot();
        let auto = if snapshot.deepseek_configured {
            Some("deepseek")
        } else if snapshot.ollama_configured {
            Some("ollama")
        } else {
            None
        };
        if let Some(choice) = auto {
            repo::settings::upsert_setting(pool, "ai_dedup.provider", choice).await?;
        } else {
            return Err(AppError::BadRequest("请先在“大模型配置”中配置 Deepseek 或 Ollama 后再启用 AI 去重".into()));
        }
    }

    get_ai_dedup_settings(pool, translator).await
}
