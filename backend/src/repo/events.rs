use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row, Postgres, QueryBuilder};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventRecord {
    pub id: i64,
    pub ts: DateTime<Utc>,
    pub level: String,
    pub code: String,
    pub source_domain: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewEvent {
    pub level: String,
    pub code: String,
    pub source_domain: Option<String>,
}

pub async fn upsert_event(pool: &PgPool, ev: &NewEvent, _window_seconds: i64) -> Result<EventRecord, sqlx::Error> {
    let inserted = sqlx::query(
        r#"
        INSERT INTO news.events (level, code, source_domain)
        VALUES ($1,$2,$3)
        RETURNING id, ts, level, code, source_domain
        "#,
    )
    .bind(&ev.level)
    .bind(&ev.code)
    .bind(&ev.source_domain)
    .fetch_one(pool)
    .await?;
    Ok(row_to_record(inserted))
}

fn row_to_record(row: sqlx::postgres::PgRow) -> EventRecord {
    EventRecord {
        id: row.get("id"),
        ts: row.get("ts"),
        level: row.get("level"),
        code: row.get("code"),
        source_domain: row.get("source_domain"),
    }
}

#[derive(Debug, Deserialize)]
pub struct ListParams {
    pub level: Option<String>,
    pub code: Option<String>,
    pub source: Option<String>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    pub since_id: Option<i64>,
    pub limit: Option<i64>,
}

pub async fn list_events(pool: &PgPool, params: &ListParams) -> Result<Vec<EventRecord>, sqlx::Error> {
    let mut qb = QueryBuilder::<Postgres>::new(
        "SELECT id, ts, level, code, source_domain FROM news.events WHERE 1=1",
    );

    if let Some(level) = &params.level {
        qb.push(" AND level = ").push_bind(level);
    }
    if let Some(code) = &params.code {
        qb.push(" AND code = ").push_bind(code);
    }
    if let Some(source) = &params.source {
        qb.push(" AND source = ").push_bind(source);
    }
    if let Some(from) = &params.from {
        qb.push(" AND ts >= ").push_bind(from);
    }
    if let Some(to) = &params.to {
        qb.push(" AND ts <= ").push_bind(to);
    }
    if let Some(since_id) = &params.since_id {
        qb.push(" AND id > ").push_bind(since_id);
    }

    qb.push(" ORDER BY ts DESC LIMIT ")
        .push_bind(params.limit.unwrap_or(50).clamp(1, 200));

    let rows = qb.build().fetch_all(pool).await?;
    Ok(rows.into_iter().map(row_to_record).collect())
}

// Deletion API removed per read-only alerts design
