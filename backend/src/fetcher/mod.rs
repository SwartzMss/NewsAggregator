use std::{collections::BTreeSet, sync::Arc, time::Duration};

use anyhow::{anyhow, Context};
use chrono::{DateTime, Utc};
use feed_rs::{model::Entry, parser};
use reqwest::{Client, StatusCode};
use tokio::{
    task::JoinSet,
    time::{interval, sleep, MissedTickBehavior},
};
use tracing::{debug, info, warn};

use crate::{
    config::{FetcherConfig, HttpClientConfig},
    repo::{
        article_sources::{self, ArticleSourceRecord},
        articles::{self, ArticleRow, NewArticle},
        feeds::{self, DueFeedRow},
    },
    util::{
        deepseek::ArticleSnippet,
        title::{jaccard_similarity, prepare_title_signature},
        translator::TranslationEngine,
        url_norm::normalize_article_url,
    },
};

struct ArticleSummary {
    article_id: i64,
    title: String,
    source_domain: String,
    url: String,
    description: Option<String>,
    published_at: DateTime<Utc>,
}

struct CandidateArticle {
    tokens: BTreeSet<String>,
    summary: ArticleSummary,
}

const TRANSLATION_LANG: &str = "zh-CN";

fn should_translate_title(title: &str) -> bool {
    if title.trim().is_empty() {
        return false;
    }

    if contains_cjk(title) {
        return false;
    }

    let mut ascii_letters = 0;
    let mut non_ascii_letters = 0;

    for ch in title.chars() {
        if ch.is_ascii_alphabetic() {
            ascii_letters += 1;
        } else if ch.is_alphabetic() {
            non_ascii_letters += 1;
        }
    }

    let total_letters = ascii_letters + non_ascii_letters;
    if total_letters == 0 {
        return false;
    }

    if ascii_letters == 0 {
        return false;
    }

    let ratio = ascii_letters as f32 / total_letters as f32;
    ratio >= 0.6
}

fn contains_cjk(value: &str) -> bool {
    value.chars().any(|ch| {
        matches!(
            ch,
            '\u{4E00}'..='\u{9FFF}'
                | '\u{3400}'..='\u{4DBF}'
                | '\u{20000}'..='\u{2A6DF}'
                | '\u{2A700}'..='\u{2B73F}'
                | '\u{2B740}'..='\u{2B81F}'
                | '\u{2B820}'..='\u{2CEAF}'
                | '\u{F900}'..='\u{FAFF}'
                | '\u{2F800}'..='\u{2FA1F}'
        )
    })
}

const STRICT_DUP_THRESHOLD: f32 = 0.9;
const DEEPSEEK_THRESHOLD: f32 = 0.6;
const RECENT_ARTICLE_LIMIT: i64 = 200;
const MAX_DEEPSEEK_CHECKS: usize = 3;

pub fn spawn(
    pool: sqlx::PgPool,
    fetcher_config: FetcherConfig,
    http_client_config: HttpClientConfig,
    translator: Arc<TranslationEngine>,
) -> anyhow::Result<()> {
    let fetcher = Fetcher::new(pool, fetcher_config, http_client_config, translator)?;
    tokio::spawn(async move {
        if let Err(err) = fetcher.run().await {
            tracing::error!(error = ?err, "fetcher stopped");
        }
    });
    Ok(())
}

pub async fn fetch_feed_once(
    pool: sqlx::PgPool,
    fetcher_config: FetcherConfig,
    http_client_config: HttpClientConfig,
    translator: Arc<TranslationEngine>,
    feed_id: i64,
) -> anyhow::Result<()> {
    let config = normalize_fetcher_config(fetcher_config);

    let client_builder = http_client_config
        .apply(Client::builder().user_agent("NewsAggregatorFetcher/0.1"))
        .context("failed to apply proxy settings for fetcher client")?
        .timeout(Duration::from_secs(config.request_timeout_secs));

    let client = Arc::new(client_builder.build()?);

    let feed = feeds::find_due_feed(&pool, feed_id)
        .await?
        .ok_or_else(|| anyhow!("feed {feed_id} not found"))?;

    let retry_delay = Duration::from_secs(config.quick_retry_delay_secs);
    process_feed(
        pool,
        client,
        translator,
        feed,
        config.quick_retry_attempts,
        retry_delay,
    )
    .await
}

fn normalize_fetcher_config(mut config: FetcherConfig) -> FetcherConfig {
    if config.interval_secs == 0 {
        config.interval_secs = 60;
    }
    if config.batch_size == 0 {
        config.batch_size = 4;
    }
    if config.concurrency == 0 {
        config.concurrency = 1;
    }
    if config.request_timeout_secs == 0 {
        config.request_timeout_secs = 10;
    }
    if config.quick_retry_attempts > 0 && config.quick_retry_delay_secs == 0 {
        config.quick_retry_delay_secs = 10;
    }
    config
}

struct Fetcher {
    pool: sqlx::PgPool,
    client: Client,
    config: FetcherConfig,
    translation: Arc<TranslationEngine>,
}

impl Fetcher {
    fn new(
        pool: sqlx::PgPool,
        config: FetcherConfig,
        http_client_config: HttpClientConfig,
        translator: Arc<TranslationEngine>,
    ) -> anyhow::Result<Self> {
        let config = normalize_fetcher_config(config);

        let client_builder = http_client_config
            .apply(Client::builder().user_agent("NewsAggregatorFetcher/0.1"))
            .context("failed to apply proxy settings for fetcher client")?
            .timeout(Duration::from_secs(config.request_timeout_secs));

        let client = client_builder.build()?;

        Ok(Self {
            pool,
            client,
            config,
            translation: translator,
        })
    }

    async fn run(self) -> anyhow::Result<()> {
        let Self {
            pool,
            client,
            config,
            translation,
        } = self;

        let client = Arc::new(client);
        let translation = Arc::clone(&translation);
        let mut ticker = interval(Duration::from_secs(config.interval_secs));
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
        ticker.tick().await; // immediate first run

        loop {
            ticker.tick().await;
            if let Err(err) = Self::run_once(
                pool.clone(),
                client.clone(),
                Arc::clone(&translation),
                &config,
            )
            .await
            {
                warn!(error = ?err, "fetcher iteration failed");
            }
        }
    }

    async fn run_once(
        pool: sqlx::PgPool,
        client: Arc<Client>,
        translation: Arc<TranslationEngine>,
        config: &FetcherConfig,
    ) -> anyhow::Result<()> {
        let feeds = feeds::list_due_feeds(&pool, config.batch_size as i64).await?;
        if feeds.is_empty() {
            debug!("no feeds eligible this round");
            return Ok(());
        }

        info!(count = feeds.len(), "starting fetch round");

        let concurrency = config.concurrency as usize;
        let mut set = JoinSet::new();
        let retry_attempts = config.quick_retry_attempts;
        let retry_delay = Duration::from_secs(config.quick_retry_delay_secs);

        for feed in feeds {
            let pool_cloned = pool.clone();
            let client_cloned = client.clone();
            let translation_cloned = Arc::clone(&translation);
            let delay = retry_delay;

            set.spawn(async move {
                debug!(feed_id = feed.id, url = %feed.url, "fetching feed");
                if let Err(err) = process_feed(
                    pool_cloned,
                    client_cloned,
                    translation_cloned,
                    feed.clone(),
                    retry_attempts,
                    delay,
                )
                .await
                {
                    warn!(
                        error = ?err,
                        feed_id = feed.id,
                        url = %feed.url,
                        "failed to process feed"
                    );
                }
            });

            if set.len() >= concurrency {
                if let Some(res) = set.join_next().await {
                    let _ = res;
                }
            }
        }

        while set.join_next().await.is_some() {}

        Ok(())
    }
}

async fn process_feed(
    pool: sqlx::PgPool,
    client: Arc<Client>,
    translation: Arc<TranslationEngine>,
    feed: DueFeedRow,
    retry_attempts: u32,
    retry_delay: Duration,
) -> anyhow::Result<()> {
    let mut lock_conn = pool.acquire().await?;
    feeds::acquire_processing_lock(&mut lock_conn, feed.id).await?;

    let feed_id = feed.id;
    let max_attempts = retry_attempts.saturating_add(1) as usize;
    let mut result = Ok(());

    for attempt in 0..max_attempts {
        let is_last = attempt + 1 == max_attempts;
        let outcome = process_feed_locked(
            pool.clone(),
            client.clone(),
            Arc::clone(&translation),
            &feed,
            is_last,
        )
        .await;

        match outcome {
            Ok(_) => {
                result = Ok(());
                break;
            }
            Err(err) => {
                result = Err(err);
                if is_last {
                    break;
                }

                debug!(
                    feed_id = feed.id,
                    url = %feed.url,
                    attempt = attempt + 1,
                    "feed fetch failed, retrying shortly"
                );

                if !retry_delay.is_zero() {
                    sleep(retry_delay).await;
                }
            }
        }
    }

    let release_result = feeds::release_processing_lock(&mut lock_conn, feed_id).await;
    drop(lock_conn);

    if let Err(err) = release_result {
        warn!(error = ?err, feed_id = feed.id, "failed to release feed lock");
        if result.is_ok() {
            return Err(err.into());
        }
    }

    result
}

async fn process_feed_locked(
    pool: sqlx::PgPool,
    client: Arc<Client>,
    translation: Arc<TranslationEngine>,
    feed: &DueFeedRow,
    persist_failure: bool,
) -> anyhow::Result<()> {
    let mut request = client.get(&feed.url);
    if let Some(etag) = &feed.last_etag {
        request = request.header(reqwest::header::IF_NONE_MATCH, etag);
    }
    let response = match request.send().await {
        Ok(resp) => resp,
        Err(err) => {
            warn!(
                feed_id = feed.id,
                url = %feed.url,
                error = %err,
                chain = %format_error_chain(&err),
                "failed to fetch feed"
            );
            record_failure(&pool, feed.id, err.status(), persist_failure).await?;
            return Err(err.into());
        }
    };

    let status = response.status();
    let headers = response.headers().clone();
    if status == StatusCode::NOT_MODIFIED {
        feeds::mark_not_modified(&pool, feed.id, status.as_u16() as i16).await?;
        debug!(
            feed_id = feed.id,
            status = status.as_u16(),
            "feed not modified"
        );
        return Ok(());
    }

    if !status.is_success() {
        record_failure(&pool, feed.id, Some(status), persist_failure).await?;
        return Err(anyhow!("unexpected status {}", status));
    }

    let bytes = match response.bytes().await {
        Ok(bytes) => bytes,
        Err(err) => {
            record_failure(&pool, feed.id, Some(status), persist_failure).await?;
            return Err(err.into());
        }
    };

    let mut parsed_feed = match parser::parse(&bytes[..]) {
        Ok(feed) => feed,
        Err(err) => {
            record_failure(&pool, feed.id, Some(status), persist_failure).await?;
            return Err(err.into());
        }
    };

    let recent_articles = articles::list_recent_articles(&pool, RECENT_ARTICLE_LIMIT).await?;
    let mut historical_candidates = Vec::new();
    for row in recent_articles {
        let ArticleRow {
            id,
            title,
            url,
            description,
            language: _,
            source_domain,
            published_at,
            click_count: _,
        } = row;

        let (_, tokens) = prepare_title_signature(&title);
        if tokens.is_empty() {
            continue;
        }
        historical_candidates.push(CandidateArticle {
            tokens,
            summary: ArticleSummary {
                article_id: id,
                title,
                source_domain,
                url,
                description,
                published_at,
            },
        });
    }

    let etag = headers
        .get(reqwest::header::ETAG)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let entries = std::mem::take(&mut parsed_feed.entries);
    let mut articles = Vec::new();
    let mut seen_signatures: Vec<(BTreeSet<String>, String)> = Vec::new();

    let deepseek = translation.deepseek_client();

    for entry in &entries {
        if let Some(mut article) = convert_entry(feed, &entry) {
            let original_title = article.title.clone();
            let original_description = article.description.clone();

            if should_translate_title(&original_title) {
                let description_input = if translation.translate_descriptions() {
                    original_description.as_deref()
                } else {
                    None
                };

                match translation
                    .translate(&original_title, description_input)
                    .await
                {
                    Ok(Some(translated)) => {
                        article.title = translated.title;
                        article.description = translated.description.or(original_description);
                        article.language = Some(TRANSLATION_LANG.to_string());
                    }
                    Ok(None) => {
                        debug!(
                            feed_id = feed.id,
                            url = %article.url,
                            "translation skipped (no provider configured)"
                        );
                    }
                    Err(err) => {
                        warn!(
                            error = %err,
                            feed_id = feed.id,
                            url = %article.url,
                            "failed to translate article"
                        );
                    }
                }
            }

            let (normalized_title, tokens) = prepare_title_signature(&article.title);

            if tokens.is_empty() {
                continue;
            }

            let mut is_duplicate = false;
            for (existing_tokens, existing_title) in &seen_signatures {
                let similarity = jaccard_similarity(&tokens, existing_tokens);
                if similarity >= STRICT_DUP_THRESHOLD {
                    is_duplicate = true;
                    debug!(
                        feed_id = feed.id,
                        similarity,
                        title = %article.title,
                        "skip article due to high intra-feed title similarity"
                    );
                    break;
                }

                if normalized_title == *existing_title {
                    is_duplicate = true;
                    debug!(
                        feed_id = feed.id,
                        title = %article.title,
                        "skip article due to identical normalized title"
                    );
                    break;
                }
            }

            if is_duplicate {
                continue;
            }

            if !historical_candidates.is_empty() {
                let mut deepseek_checks = 0usize;
                for candidate in &historical_candidates {
                    let similarity = jaccard_similarity(&tokens, &candidate.tokens);
                    if similarity >= STRICT_DUP_THRESHOLD {
                        record_article_source(
                            &pool,
                            feed,
                            &article,
                            candidate.summary.article_id,
                            Some("recent_jaccard"),
                            Some(similarity),
                        )
                        .await;
                        is_duplicate = true;
                        info!(
                            feed_id = feed.id,
                            similarity,
                            title = %article.title,
                            existing_article_id = candidate.summary.article_id,
                            existing_title = %candidate.summary.title,
                            existing_url = %candidate.summary.url,
                            existing_source = %candidate.summary.source_domain,
                            "skip article due to matching recent article"
                        );
                        break;
                    }

                    if similarity >= DEEPSEEK_THRESHOLD {
                        if let Some(client) = deepseek.as_ref() {
                            if deepseek_checks >= MAX_DEEPSEEK_CHECKS {
                                break;
                            }
                            deepseek_checks += 1;

                            let published_new = article.published_at.to_rfc3339();
                            let published_existing = candidate.summary.published_at.to_rfc3339();

                            let new_snippet = ArticleSnippet {
                                title: &article.title,
                                source: Some(&article.source_domain),
                                url: Some(&article.url),
                                published_at: Some(&published_new),
                                summary: article.description.as_deref(),
                            };

                            let existing_summary_ref = candidate.summary.description.as_deref();
                            let existing_snippet = ArticleSnippet {
                                title: &candidate.summary.title,
                                source: Some(&candidate.summary.source_domain),
                                url: Some(&candidate.summary.url),
                                published_at: Some(&published_existing),
                                summary: existing_summary_ref,
                            };

                            match client
                                .judge_similarity(&new_snippet, &existing_snippet)
                                .await
                            {
                                Ok(decision) => {
                                    if decision.is_duplicate {
                                        let reason = decision
                                            .reason
                                            .as_deref()
                                            .unwrap_or("deepseek_duplicate");
                                        record_article_source(
                                            &pool,
                                            feed,
                                            &article,
                                            candidate.summary.article_id,
                                            Some(reason),
                                            decision.confidence,
                                        )
                                        .await;
                                        is_duplicate = true;
                                        info!(
                                            feed_id = feed.id,
                                            title = %article.title,
                                            existing_article_id = candidate.summary.article_id,
                                            existing_title = %candidate.summary.title,
                                            existing_url = %candidate.summary.url,
                                            existing_source = %candidate.summary.source_domain,
                                            reason = decision.reason.as_deref().unwrap_or(""),
                                            "skip article due to deepseek duplicate judgment"
                                        );
                                        break;
                                    }
                                }
                                Err(err) => {
                                    warn!(
                                        error = ?err,
                                        feed_id = feed.id,
                                        "deepseek similarity check failed"
                                    );
                                }
                            }
                        }
                    }
                }

                if is_duplicate {
                    continue;
                }
            }

            seen_signatures.push((tokens.clone(), normalized_title.clone()));
            articles.push(article);
        }
    }

    let article_count = articles.len();
    if article_count > 0 {
        let inserted = articles::insert_articles(&pool, articles).await?;
        for (article_id, article) in &inserted {
            record_article_source(&pool, feed, article, *article_id, Some("primary"), None).await;
        }
        if let Some(condition) = feed
            .filter_condition
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            match articles::apply_filter_condition(&pool, feed.id, condition).await {
                Ok(deleted) => {
                    if deleted > 0 {
                        info!(
                            feed_id = feed.id,
                            deleted, "filtered articles using feed condition"
                        );
                    }
                }
                Err(err) => {
                    warn!(
                        error = ?err,
                        feed_id = feed.id,
                        "failed to apply feed filter condition"
                    );
                }
            }
        }
        info!(
            feed_id = feed.id,
            count = article_count,
            "inserted articles"
        );
    } else {
        debug!(feed_id = feed.id, "no new articles parsed");
    }

    let title = parsed_feed.title.as_ref().map(|text| text.content.clone());

    let site_url = parsed_feed.links.first().map(|link| link.href.clone());

    feeds::mark_success(
        &pool,
        feed.id,
        status.as_u16() as i16,
        etag,
        title,
        site_url,
    )
    .await?;

    info!(
        feed_id = feed.id,
        status = status.as_u16(),
        last_fetch_at = ?Utc::now(),
        "feed fetch successful"
    );

    Ok(())
}

fn format_error_chain(err: &(dyn std::error::Error + 'static)) -> String {
    let mut parts = vec![err.to_string()];
    let mut current = err.source();

    while let Some(source) = current {
        parts.push(source.to_string());
        current = source.source();
    }

    parts.join(" -> ")
}

async fn record_article_source(
    pool: &sqlx::PgPool,
    feed: &DueFeedRow,
    article: &NewArticle,
    article_id: i64,
    decision: Option<&str>,
    confidence: Option<f32>,
) {
    let record = ArticleSourceRecord {
        article_id,
        feed_id: Some(feed.id),
        source_name: Some(feed.source_domain.clone()),
        source_url: article.url.clone(),
        published_at: article.published_at,
        decision: decision.map(|s| s.to_string()),
        confidence,
    };

    if let Err(err) = article_sources::insert_source(pool, record).await {
        warn!(
            error = ?err,
            feed_id = feed.id,
            article_id,
            "failed to record article source"
        );
    }
}

fn convert_entry(feed: &DueFeedRow, entry: &Entry) -> Option<NewArticle> {
    let title = entry.title.as_ref()?.content.trim();
    if title.is_empty() {
        return None;
    }

    let link = entry
        .links
        .iter()
        .find(|link| link.rel.as_deref() == Some("alternate"))
        .or_else(|| entry.links.first())?;
    let raw_url = link.href.clone();
    let url = match normalize_article_url(&raw_url) {
        Ok(normalized) => normalized,
        Err(err) => {
            warn!(error = ?err, url = %raw_url, "failed to normalize article url");
            raw_url
        }
    };

    let description = entry
        .summary
        .as_ref()
        .map(|summary| summary.content.clone())
        .filter(|s| !s.trim().is_empty());

    let language = entry.language.clone();

    let published_at = entry
        .published
        .clone()
        .or_else(|| entry.updated.clone())
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(Utc::now);

    Some(NewArticle {
        feed_id: Some(feed.id),
        title: title.to_string(),
        url,
        description,
        language,
        source_domain: feed.source_domain.clone(),
        published_at,
    })
}

async fn record_failure(
    pool: &sqlx::PgPool,
    feed_id: i64,
    http_status: Option<StatusCode>,
    persist: bool,
) -> anyhow::Result<()> {
    let status = http_status.map(|s| s.as_u16() as i16).unwrap_or(0);
    if persist {
        feeds::mark_failure(pool, feed_id, status).await?;
        warn!(feed_id, status, "marked feed fetch failure");
    } else {
        debug!(
            feed_id,
            status, "feed fetch failed, will attempt quick retry"
        );
    }
    Ok(())
}
