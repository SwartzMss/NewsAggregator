use sqlx::PgPool;

pub async fn get_setting(pool: &PgPool, key: &str) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar::<_, String>(
        r#"
        SELECT value
        FROM news.settings
        WHERE key = $1
        "#,
    )
    .bind(key)
    .fetch_optional(pool)
    .await
}

pub async fn upsert_setting(pool: &PgPool, key: &str, value: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO news.settings (key, value)
        VALUES ($1, $2)
        ON CONFLICT (key) DO UPDATE
        SET value = EXCLUDED.value,
            updated_at = NOW()
        "#,
    )
    .bind(key)
    .bind(value)
    .execute(pool)
    .await
    .map(|_| ())
}

pub async fn delete_setting(pool: &PgPool, key: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        DELETE FROM news.settings
        WHERE key = $1
        "#,
    )
    .bind(key)
    .execute(pool)
    .await
    .map(|_| ())
}
