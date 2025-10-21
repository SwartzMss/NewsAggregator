use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row, Postgres, QueryBuilder};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventRecord {
    pub id: i64,
    pub ts: DateTime<Utc>,
    pub level: String,
    pub code: String,
    pub title: String,
    pub message: String,
    pub attrs: serde_json::Value,
    pub source: String,
    pub dedupe_key: Option<String>,
    pub count: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewEvent {
    pub level: String,
    pub code: String,
    pub title: String,
    pub message: String,
    pub attrs: serde_json::Value,
    pub source: String,
    pub dedupe_key: Option<String>,
}

pub async fn upsert_event(pool: &PgPool, ev: &NewEvent, window_seconds: i64) -> Result<EventRecord, sqlx::Error> {
    // try update latest row in time window with same (code, dedupe_key)
    let updated = sqlx::query(
        r#"
        UPDATE ops.events
        SET count = count + 1, ts = NOW(), level = $1, title = $2, message = $3, attrs = $4, source = $5
        WHERE id = (
          SELECT id FROM ops.events
          WHERE code = $6 AND ((dedupe_key IS NULL AND $7 IS NULL) OR dedupe_key = $7)
            AND ts >= NOW() - make_interval(secs := $8)
          ORDER BY ts DESC
          LIMIT 1
        )
        RETURNING id, ts, level, code, title, message, attrs, source, dedupe_key, count
        "#,
    )
    .bind(&ev.level)
    .bind(&ev.title)
    .bind(&ev.message)
    .bind(&ev.attrs)
    .bind(&ev.source)
    .bind(&ev.code)
    .bind(&ev.dedupe_key)
    .bind(window_seconds)
    .fetch_optional(pool)
    .await?;

    if let Some(row) = updated {
        return Ok(row_to_record(row));
    }

    let inserted = sqlx::query(
        r#"
        INSERT INTO ops.events (level, code, title, message, attrs, source, dedupe_key)
        VALUES ($1,$2,$3,$4,$5,$6,$7)
        RETURNING id, ts, level, code, title, message, attrs, source, dedupe_key, count
        "#,
    )
    .bind(&ev.level)
    .bind(&ev.code)
    .bind(&ev.title)
    .bind(&ev.message)
    .bind(&ev.attrs)
    .bind(&ev.source)
    .bind(&ev.dedupe_key)
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
        title: row.get("title"),
        message: row.get("message"),
        attrs: row.get("attrs"),
        source: row.get("source"),
        dedupe_key: row.get("dedupe_key"),
        count: row.get("count"),
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
        "SELECT id, ts, level, code, title, message, attrs, source, dedupe_key, count FROM ops.events WHERE 1=1",
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

pub async fn delete_events_by_ids(pool: &PgPool, ids: &[i64]) -> Result<u64, sqlx::Error> {
    if ids.is_empty() { return Ok(0); }
    let mut qb = QueryBuilder::<Postgres>::new("DELETE FROM ops.events WHERE id = ANY(");
    qb.push_bind(ids).push(")");
    let res = qb.build().execute(pool).await?;
    Ok(res.rows_affected())
}
