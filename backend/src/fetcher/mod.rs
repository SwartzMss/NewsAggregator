use std::{sync::Arc, time::Duration};

use anyhow::anyhow;
use chrono::Utc;
use feed_rs::{model::Entry, parser};
use reqwest::{Client, StatusCode};
use tokio::{
    task::JoinSet,
    time::{interval, MissedTickBehavior},
};
use tracing::{debug, info, warn};

use std::collections::BTreeSet;

use crate::{
    config::FetcherConfig,
    repo::{
        articles::{self, NewArticle},
        feeds::{self, DueFeedRow},
    },
    util::{
        title::{jaccard_similarity, prepare_title_signature},
        url_norm::normalize_article_url,
    },
};

pub fn spawn(pool: sqlx::PgPool, config: FetcherConfig) -> anyhow::Result<()> {
    let fetcher = Fetcher::new(pool, config)?;
    tokio::spawn(async move {
        if let Err(err) = fetcher.run().await {
            tracing::error!(error = ?err, "fetcher stopped");
        }
    });
    Ok(())
}

struct Fetcher {
    pool: sqlx::PgPool,
    client: Client,
    config: FetcherConfig,
}

impl Fetcher {
    fn new(pool: sqlx::PgPool, mut config: FetcherConfig) -> anyhow::Result<Self> {
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

        let client = Client::builder()
            .user_agent("NewsAggregatorFetcher/0.1")
            .timeout(Duration::from_secs(config.request_timeout_secs))
            .build()?;

        Ok(Self {
            pool,
            client,
            config,
        })
    }

    async fn run(self) -> anyhow::Result<()> {
        let Self {
            pool,
            client,
            config,
        } = self;

        let client = Arc::new(client);
        let mut ticker = interval(Duration::from_secs(config.interval_secs));
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
        ticker.tick().await; // immediate first run

        loop {
            ticker.tick().await;
            if let Err(err) = Self::run_once(pool.clone(), client.clone(), &config).await {
                warn!(error = ?err, "fetcher iteration failed");
            }
        }
    }

    async fn run_once(
        pool: sqlx::PgPool,
        client: Arc<Client>,
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

        for feed in feeds {
            let pool_cloned = pool.clone();
            let client_cloned = client.clone();

            set.spawn(async move {
                debug!(feed_id = feed.id, url = %feed.url, "fetching feed");
                if let Err(err) = process_feed(pool_cloned, client_cloned, feed).await {
                    warn!(error = ?err, "failed to process feed");
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
    feed: DueFeedRow,
) -> anyhow::Result<()> {
    let mut request = client.get(&feed.url);
    if let Some(etag) = &feed.last_etag {
        request = request.header(reqwest::header::IF_NONE_MATCH, etag);
    }
    let response = match request.send().await {
        Ok(resp) => resp,
        Err(err) => {
            record_failure(&pool, feed.id, err.status()).await?;
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
        feeds::mark_failure(&pool, feed.id, status.as_u16() as i16).await?;
        return Err(anyhow!("unexpected status {}", status));
    }

    let bytes = match response.bytes().await {
        Ok(bytes) => bytes,
        Err(err) => {
            feeds::mark_failure(&pool, feed.id, status.as_u16() as i16).await?;
            return Err(err.into());
        }
    };

    let mut parsed_feed = match parser::parse(&bytes[..]) {
        Ok(feed) => feed,
        Err(err) => {
            feeds::mark_failure(&pool, feed.id, status.as_u16() as i16).await?;
            return Err(err.into());
        }
    };

    let etag = headers
        .get(reqwest::header::ETAG)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let entries = std::mem::take(&mut parsed_feed.entries);
    let mut articles = Vec::new();
    let mut seen_signatures: Vec<(BTreeSet<String>, String)> = Vec::new();

    for entry in &entries {
        if let Some(article) = convert_entry(&feed, &entry) {
            let (normalized_title, tokens) = prepare_title_signature(&article.title);

            let mut is_duplicate = false;
            for (existing_tokens, existing_title) in &seen_signatures {
                if !tokens.is_empty() && !existing_tokens.is_empty() {
                    let similarity = jaccard_similarity(&tokens, existing_tokens);
                    if similarity >= 0.9 {
                        is_duplicate = true;
                        debug!(
                            feed_id = feed.id,
                            similarity = similarity,
                            title = %article.title,
                            "skip article due to high title similarity"
                        );
                        break;
                    }
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

            seen_signatures.push((tokens, normalized_title));
            articles.push(article);
        }
    }

    let article_count = articles.len();
    if article_count > 0 {
        articles::insert_articles(&pool, articles).await?;
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
) -> anyhow::Result<()> {
    let status = http_status.map(|s| s.as_u16() as i16).unwrap_or(0);
    feeds::mark_failure(pool, feed_id, status).await?;
    warn!(feed_id, status, "marked feed fetch failure");
    Ok(())
}
