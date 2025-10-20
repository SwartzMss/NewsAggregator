use std::{collections::BTreeSet, sync::Arc, time::Duration};

// 抓取器（Fetcher）模块：
// 负责周期性地抓取订阅源（RSS/Atom）内容，进行：
// 1. 网络请求（支持代理与超时）
// 2. 条目解析与字段规范化（URL 归一化、发布时间提取）
// 3. 标题去重（同一批次内 + 与最近历史文章）
// 4. 可选的标题与摘要翻译（多翻译提供者级联，失败重试一次）
// 5. 基于 Jaccard 相似度 + LLM（Deepseek/Ollama）判断跨文章重复
// 6. 入库（文章主表 + 来源追踪表）与失败状态标记
// 7. 支持快速重试与并发抓取控制
//
// 设计目标：稳定、可观察（丰富 tracing 日志）、对失败具备自恢复能力，避免重复与垃圾内容进入主库。

use anyhow::{anyhow, Context};
use chrono::{DateTime, Utc};
use feed_rs::{model::Entry, parser};
use reqwest::{Client, StatusCode};
use tokio::{
    task::JoinSet,
    time::{interval, sleep, timeout, MissedTickBehavior},
};
use tracing::{info, warn};

use crate::{
    config::{FetcherConfig, HttpClientConfig},
    repo::{
        article_sources::{self, ArticleSourceRecord},
        articles::{self, ArticleRow, NewArticle},
        feeds::{self, DueFeedRow},
        settings,
    },
    util::{
        deepseek::ArticleSnippet,
        html::strip_html_basic,
        title::{jaccard_similarity, prepare_title_signature},
        translator::TranslationEngine,
        url_norm::normalize_article_url,
    },
};

// 最近文章的简要信息，用于与当前抓取文章做相似度比较
struct ArticleSummary {
    article_id: i64,
    title: String,
    source_domain: String,
    url: String,
    description: Option<String>,
    published_at: DateTime<Utc>,
}

// 候选文章：预先分词后的 Token 集合 + 摘要
struct CandidateArticle {
    tokens: BTreeSet<String>,
    summary: ArticleSummary,
}

const TRANSLATION_LANG: &str = "zh-CN";

// 轻量级 HTML 实体解码：
// 支持常见命名实体与十进制/十六进制数字实体，避免引入额外依赖。
fn html_unescape_minimal(input: &str) -> String {
    // 快速路径：没有'&'则直接返回原字符串拷贝
    if !input.as_bytes().contains(&b'&') {
        return input.to_string();
    }

    let mut out = String::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'&' {
            // 查找下一个分号
            if let Some(semi) = bytes[i + 1..].iter().position(|&b| b == b';') {
                let end = i + 1 + semi; // 分号前位置
                let entity = &input[i + 1..=end]; // 含分号
                let decoded = match entity {
                    // 常见命名实体（含分号）
                    "amp;" => Some('&'),
                    "lt;" => Some('<'),
                    "gt;" => Some('>'),
                    "quot;" => Some('"'),
                    "apos;" => Some('\''),
                    // 一些源里会出现没有分号的奇怪情况，这里不处理以避免误判
                    _ => {
                        // 数字实体：十进制 &#NNN; 或 十六进制 &#xHHH;
                        if let Some(rest) = entity.strip_prefix("#x") {
                            // 十六进制
                            let hex = rest.trim_end_matches(';');
                            if let Ok(code) = u32::from_str_radix(hex, 16) {
                                std::char::from_u32(code)
                            } else {
                                None
                            }
                        } else if let Some(rest) = entity.strip_prefix('#') {
                            // 十进制
                            let dec = rest.trim_end_matches(';');
                            if let Ok(code) = dec.parse::<u32>() {
                                std::char::from_u32(code)
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    }
                };

                if let Some(ch) = decoded {
                    out.push(ch);
                    i = end + 2; // 跳过 &...[;]
                    continue;
                }
            }
        }
        // 常规字符或未识别实体，原样写入
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

fn should_translate_title(title: &str) -> bool {
    // 翻译判定逻辑：
    // 1. 空标题不翻译
    // 2. 已包含 CJK（中文、日文、韩文统一表意字符）则认为不需要翻译
    // 3. 统计 ASCII 字母 vs 非 ASCII 字母比例，避免纯符号或数字
    // 4. ASCII 比例 >= 0.6 认为是英文主导，触发翻译
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

// Jaccard 严格重复阈值：>= 0.9 判定为几乎完全重复
const STRICT_DUP_THRESHOLD: f32 = 0.9;
// 触发 LLM 深度相似度判定的较宽松阈值：>= 0.6 进入 Deepseek 检查
const DEEPSEEK_THRESHOLD: f32 = 0.6;
// 最近历史文章数量上限：控制比较规模与性能
const RECENT_ARTICLE_LIMIT: i64 = 100;
// 对单篇新文章进行 LLM 相似度检查的最大次数（防止成本与延迟爆炸）
const MAX_DEEPSEEK_CHECKS: usize = 3;

pub fn spawn(
    pool: sqlx::PgPool,
    fetcher_config: FetcherConfig,
    http_client_config: HttpClientConfig,
    translator: Arc<TranslationEngine>,
) -> anyhow::Result<()> {
    // 后台启动永久运行的抓取循环任务
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
    // 对用户配置进行兜底规范：避免出现 0 导致逻辑停滞或请求无超时
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
    ticker.tick().await; // 立即执行一次（不等待第一个间隔）

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
                // 单轮抓取失败记录日志，但不退出主循环（保持自恢复）
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
            info!("no feeds eligible this round");
            return Ok(());
        }

        info!(count = feeds.len(), "starting fetch round");

        let concurrency = config.concurrency as usize;
        let mut set = JoinSet::new();
        let retry_attempts = config.quick_retry_attempts;
        let retry_delay = Duration::from_secs(config.quick_retry_delay_secs);

        for feed in feeds {
            // 每个 feed 使用 tokio JoinSet 并发处理，受 concurrency 限制
            let pool_cloned = pool.clone();
            let client_cloned = client.clone();
            let translation_cloned = Arc::clone(&translation);
            let delay = retry_delay;

            set.spawn(async move {
                info!(feed_id = feed.id, url = %feed.url, "fetching feed");
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
    // 非阻塞尝试获取分布式/数据库级锁；若未获取到，说明该 feed 正在处理，直接跳过本轮
    if !feeds::try_acquire_processing_lock(&mut lock_conn, feed.id).await? {
        info!(feed_id = feed.id, url = %feed.url, "feed busy, skip this round");
        return Ok(());
    }

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
                // 成功：记录成功尝试次数（attempt 从 0 开始，展示为 attempt+1）
                info!(
                    feed_id = feed.id,
                    url = %feed.url,
                    attempt = attempt + 1,
                    max_attempts,
                    "feed fetch succeeded"
                );
                result = Ok(());
                break;
            }
            Err(err) => {
                let err_for_log = err.to_string();
                result = Err(err);
                if is_last {
                    // 最后一次失败：打印错误并结束，不再重试
                    warn!(
                        feed_id = feed.id,
                        url = %feed.url,
                        attempt = attempt + 1,
                        error = %err_for_log,
                        "feed fetch failed, all retry attempts exhausted"
                    );
                    break;
                } else {
                    // 仍有剩余重试次数：打印错误并等待重试
                    info!(
                        feed_id = feed.id,
                        url = %feed.url,
                        attempt = attempt + 1,
                        error = %err_for_log,
                        "feed fetch failed, retrying shortly"
                    );
                    if !retry_delay.is_zero() {
                        sleep(retry_delay).await;
                    }
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
    // 使用 ETag 支持服务器端增量更新：未修改则快速跳过
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
        info!(
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

    info!(
        feed_id = feed.id,
        status = status.as_u16(),
        url = %feed.url,
        "feed http fetch succeeded"
    );

    let bytes = match response.bytes().await {
        Ok(bytes) => bytes,
        Err(err) => {
            record_failure(&pool, feed.id, Some(status), persist_failure).await?;
            return Err(err.into());
        }
    };

    let mut parsed_feed = match parser::parse(&bytes[..]) {
        Ok(feed) => {
            let entry_count = feed.entries.len();
            info!(
                feed_id = feed.id,
                status = status.as_u16(),
                entry_count,
                bytes_len = bytes.len(),
                "feed xml parsed"
            );
            feed
        }
        Err(err) => {
            record_failure(&pool, feed.id, Some(status), persist_failure).await?;
            return Err(err.into());
        }
    };

    let recent_articles = articles::list_recent_articles(&pool, RECENT_ARTICLE_LIMIT).await?;
    // 读取 AI 去重设置（简单每次请求一次；后续可缓存优化）
    let ai_dedup_enabled = settings::get_setting(&pool, "ai_dedup.enabled")
        .await?
        .map(|v| v == "true")
        .unwrap_or(false);
    let ai_dedup_provider = settings::get_setting(&pool, "ai_dedup.provider").await?;
    // 构造历史候选集合（近期文章做近似重复检测）
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

    for entry in &entries {
        if let Some(mut article) = convert_entry(feed, &entry) {
            let original_title = article.title.clone();

            // 提前归一化：空或全空白描述直接设为 None，避免后续重复判空
            if let Some(desc) = &article.description {
                if desc.trim().is_empty() {
                    article.description = None;
                }
            }

            // 无论是否需要翻译，都记录一次判定结果日志
            let need_translate = should_translate_title(&original_title);
            info!(
                feed_id = feed.id,
                url = %article.url,
                need_translate,
                title = %original_title,
                "title translation decision"
            );

            // 进入条目处理主流程，便于定位卡点
            info!(feed_id = feed.id, url = %article.url, "begin entry processing");

            if need_translate {
                if !translation.translation_enabled() {
                    info!(
                        feed_id = feed.id,
                        url = %article.url,
                        "translation disabled globally, skipping"
                    );
                    // 不进行翻译但保留原始标题/描述
                } else {
                // 翻译流程：根据是否翻译摘要决定传入 description；若无可用 provider 返回 None
                let translate_desc_flag = translation.translate_descriptions();
                let has_original_desc = article.description.is_some();

                info!(
                    feed_id = feed.id,
                    url = %article.url,
                    translate_descriptions = translate_desc_flag,
                    has_original_description = has_original_desc,
                    "pre-translation decision"
                );

                let desc_owned = if translate_desc_flag { article.description.clone() } else { None };

                // 开始进行翻译调用：记录 provider 与是否带描述
                info!(
                    feed_id = feed.id,
                    url = %article.url,
                    title = %original_title,
                    include_description = desc_owned.is_some(),
                    provider = ?translation.current_provider(),
                    "translation start"
                );

                match translation
                    .translate(&original_title, desc_owned.as_deref())
                    .await
                {
                    Ok(Some(translated)) => {
                        // 成功翻译：更新标题；仅在返回描述时覆盖原描述
                        article.title = translated.title;
                        if translated.description.is_some() {
                            article.description = translated.description;
                        }
                        article.language = Some(TRANSLATION_LANG.to_string());

                        if translate_desc_flag && has_original_desc && desc_owned.is_some() && article.description.is_none() {
                            warn!(
                                feed_id = feed.id,
                                url = %article.url,
                                "translator returned no description while description translation is enabled"
                            );
                        }
                    }
                    Ok(None) => {
                        let provider = translation.current_provider().as_str();
                        let provider_available = match provider {
                            "deepseek" => translation.is_deepseek_available(),
                            "ollama" => translation.ollama_client().is_some(),
                            _ => false,
                        };
                        info!(
                            feed_id = feed.id,
                            url = %article.url,
                            provider = provider,
                            provider_available,
                            "translation skipped (provider unavailable)"
                        );
                    }
                    Err(err) => {
                        // 第一次失败后短暂重试一次，降低瞬时网络抖动影响
                        warn!(
                            error = %err,
                            feed_id = feed.id,
                            url = %article.url,
                            "failed to translate article, will retry once"
                        );
                        // 一次失败重试（短暂延迟后再试一次）
                        sleep(Duration::from_millis(300)).await;
                        match translation
                            .translate(&original_title, desc_owned.as_deref())
                            .await
                        {
                            Ok(Some(translated)) => {
                                article.title = translated.title;
                                if translated.description.is_some() {
                                    article.description = translated.description;
                                }
                                article.language = Some(TRANSLATION_LANG.to_string());
                            }
                            Ok(None) => {
                                info!(
                                    feed_id = feed.id,
                                    url = %article.url,
                                    "translation skipped after retry (no provider configured)"
                                );
                            }
                            Err(err2) => {
                                warn!(
                                    error = %err2,
                                    feed_id = feed.id,
                                    url = %article.url,
                                    "failed to translate article after retry"
                                );
                            }
                        }
                    }
                }
            }
            }
            // 为单条条目处理添加硬超时，防止个别条目卡住影响整批
            let entry_timeout = Duration::from_secs(2);
            let entry_url_clone = article.url.clone();
            let result = timeout(entry_timeout, async {
                // 标记准备开始做标题签名，以区别于签名计算内部耗时
                info!(feed_id = feed.id, url = %article.url, "preparing title signature");
                let (normalized_title, tokens) = prepare_title_signature(&article.title);
                info!(feed_id = feed.id, url = %article.url, "prepared title signature");

                if tokens.is_empty() {
                    info!(feed_id = feed.id, url = %article.url, "skip entry: empty tokens after normalization");
                    return Ok::<bool, ()>(true); // treat as handled (skipped)
                }

                let mut is_duplicate = false;
                for (existing_tokens, existing_title) in &seen_signatures {
                    // 同一批次内部去重：严格 Jaccard + 归一化标题匹配
                    let similarity = jaccard_similarity(&tokens, existing_tokens);
                    if similarity >= STRICT_DUP_THRESHOLD {
                        is_duplicate = true;
                        info!(
                            feed_id = feed.id,
                            similarity,
                            title = %article.title,
                            "skip article due to high intra-feed title similarity"
                        );
                        break;
                    }

                    if normalized_title == *existing_title {
                        is_duplicate = true;
                        info!(
                            feed_id = feed.id,
                            title = %article.title,
                            "skip article due to identical normalized title"
                        );
                        break;
                    }
                }

                if is_duplicate {
                    return Ok(true);
                }

                // 批内比较结束
                info!(feed_id = feed.id, url = %article.url, checked = seen_signatures.len(), "intra-batch compare done");

                // 让出调度，避免长时间计算阻塞日志刷新
                tokio::task::yield_now().await;

                if !historical_candidates.is_empty() {
                    info!(feed_id = feed.id, url = %article.url, candidates = historical_candidates.len(), "start historical dedup compare");
                    let mut deepseek_checks = 0usize;
                    let mut candidate_counter = 0usize;
                    for candidate in &historical_candidates {
                        candidate_counter += 1;
                        let similarity = jaccard_similarity(&tokens, &candidate.tokens);
                        if candidate_counter % 25 == 0 {
                            info!(feed_id = feed.id, url = %article.url, checked = candidate_counter, similarity_hint = similarity, "dedup progress");
                        }
                    if similarity >= STRICT_DUP_THRESHOLD {
                        // 与历史文章严格匹配：直接标记来源并跳过
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

                    if ai_dedup_enabled && similarity >= DEEPSEEK_THRESHOLD {
                        // 根据配置选择模型客户端（不做自动校验）
                        let mut selected_provider = None;
                        let mut client_ollama = None;
                        let mut client_deepseek = None;
                        if let Some(provider_name) = ai_dedup_provider.as_deref() {
                            match provider_name {
                                "deepseek" => {
                                    client_deepseek = translation.deepseek_client();
                                    if client_deepseek.is_some() { selected_provider = Some("deepseek"); }
                                }
                                "ollama" => {
                                    client_ollama = translation.ollama_client();
                                    if client_ollama.is_some() { selected_provider = Some("ollama"); }
                                }
                                _ => {
                                    // 不支持的 provider，直接跳过
                                }
                            }
                        }

                        if selected_provider.is_none() {
                            info!(
                                feed_id = feed.id,
                                title = %article.title,
                                similarity,
                                ai_dedup_enabled,
                                ai_dedup_provider = ai_dedup_provider.as_deref().unwrap_or(""),
                                "llm dedup skipped (provider unavailable)"
                            );
                            continue;
                        }

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

                            let started = std::time::Instant::now();
                            info!(
                                feed_id = feed.id,
                                title = %article.title,
                                existing_article_id = candidate.summary.article_id,
                                ai_dedup_enabled,
                                ai_dedup_provider = selected_provider.unwrap_or(""),
                                "llm dedup check start"
                            );
                            // Hard cap LLM check duration to avoid long hangs
                            let timeout_secs: u64 = 10;
                            let fut = async {
                                if selected_provider == Some("deepseek") {
                                    if let Some(c) = client_deepseek.as_ref() {
                                        c.judge_similarity(&new_snippet, &existing_snippet).await
                                    } else {
                                        Err(anyhow!("deepseek provider unavailable"))
                                    }
                                } else if selected_provider == Some("ollama") {
                                    if let Some(c) = client_ollama.as_ref() {
                                        c.judge_similarity(&new_snippet, &existing_snippet).await
                                    } else {
                                        Err(anyhow!("ollama provider unavailable"))
                                    }
                                } else {
                                    Err(anyhow!("unknown provider"))
                                }
                            };
                            match timeout(Duration::from_secs(timeout_secs), fut)
                            .await
                            .map_err(|_| anyhow!("llm judge_similarity timed out in {}s", timeout_secs))
                            .and_then(|r| r.map_err(anyhow::Error::from))
                            {
                                Ok(decision) => {
                                    let elapsed_ms = started.elapsed().as_millis() as u64;
                                    info!(
                                        feed_id = feed.id,
                                        title = %article.title,
                                        existing_article_id = candidate.summary.article_id,
                                        elapsed_ms,
                                        is_duplicate = decision.is_duplicate,
                                        ai_dedup_provider = selected_provider.unwrap_or(""),
                                        "llm dedup check done"
                                    );
                                    if decision.is_duplicate {
                                        // LLM 判定重复：记录来源与理由（reason）
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
                                            ai_dedup_provider = selected_provider.unwrap_or(""),
                                            "skip article due to llm duplicate judgment"
                                        );
                                        break;
                                    }
                                }
                                Err(err) => {
                                    let elapsed_ms = started.elapsed().as_millis() as u64;
                                    warn!(
                                        error = ?err,
                                        feed_id = feed.id,
                                        elapsed_ms,
                                        ai_dedup_provider = selected_provider.unwrap_or(""),
                                        "llm dedup check failed"
                                    );
                                }
                            }
                        }
                    }
                } else {
                    info!(feed_id = feed.id, url = %article.url, "no historical candidates; skipping hist compare");
                }

                if is_duplicate {
                    return Ok(true);
                }
                Ok(false)
            }).await;
            match result {
                Ok(Ok(skipped)) => {
                    if skipped { continue; }
                }
                Ok(Err(_)) => {
                    warn!(feed_id = feed.id, url = %entry_url_clone, "entry processing aborted");
                    continue;
                }
                Err(_) => {
                    warn!(feed_id = feed.id, url = %entry_url_clone, "entry processing timed out; skip");
                    continue;
                }
            }

            info!(feed_id = feed.id, url = %article.url, "entry processing completed; proceeding to persist");

            // 入库前的数据快照（仅日志，不修改数据）
            // 安全截断描述，按字符边界避免 UTF-8 切片 panic
            let preview_desc_owned: String = article
                .description
                .as_deref()
                .map(|s| s.chars().take(80).collect::<String>())
                .unwrap_or_default();
            info!(
                feed_id = feed.id,
                url = %article.url,
                language = %article.language.as_deref().unwrap_or(""),
                preview_desc = %preview_desc_owned,
                "pre-insert article snapshot"
            );

            let (normalized_title2, tokens2) = prepare_title_signature(&article.title);
            seen_signatures.push((tokens2, normalized_title2));
            articles.push(article);
            info!(feed_id = feed.id, url = %articles.last().unwrap().url, "entry dedup finished");
        }
        // close the for-entry loop
    }

    let article_count = articles.len();
    if article_count > 0 {
        info!(feed_id = feed.id, count = article_count, "about to insert parsed articles");
        let inserted = articles::insert_articles(&pool, articles).await?;
        info!(feed_id = feed.id, inserted = inserted.len(), "articles insert finished");
        for (article_id, article) in &inserted {
            // primary 决策：来源于当前 feed 的主插入
            record_article_source(&pool, feed, article, *article_id, Some("primary"), None).await;
        }
        if let Some(condition) = feed
            .filter_condition
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            info!(feed_id = feed.id, "applying feed filter condition");
            match articles::apply_filter_condition(&pool, feed.id, condition).await {
                Ok(deleted) => {
                    if deleted > 0 {
                        info!(
                            feed_id = feed.id,
                            deleted, "filtered articles using feed condition"
                        );
                    }
                    info!(feed_id = feed.id, "feed filter condition applied");
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
        info!(feed_id = feed.id, "no new articles parsed");
    }

    let title = parsed_feed.title.as_ref().map(|text| text.content.clone());

    let site_url = parsed_feed.links.first().map(|link| link.href.clone());

    info!(feed_id = feed.id, "marking feed success");
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
    // 展开错误链，便于日志中追踪底层原因
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
    // 将 feed_rs 的 Entry 转换为内部 NewArticle 结构
    // 处理标题、链接、描述、语言与发布时间（优先 published，其次 updated，最后当前时间）
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

    // 处理标题与摘要：
    // 1) 先做基础 HTML 去标签，避免 RSS/Atom 的富文本摘要渗透
    // 2) 再做最小化 HTML 实体解码，避免 B&amp;M 等问题
    // 标题仅做实体解码，不进行 HTML 去标签（避免过度清理影响显示）
    let title = html_unescape_minimal(title);
    let description = description.map(|s| {
        let stripped = strip_html_basic(s.trim());
        html_unescape_minimal(stripped.as_str())
    });

    Some(NewArticle {
        feed_id: Some(feed.id),
        title,
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
        // 持久记录失败（超过快速重试次数或不再重试）
        feeds::mark_failure(pool, feed_id, status).await?;
        warn!(feed_id, status, "marked feed fetch failure");
    } else {
        info!(
            feed_id,
            status, "feed fetch failed, will attempt quick retry"
        );
    }
    Ok(())
}
