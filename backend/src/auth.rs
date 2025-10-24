use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use axum::{
    http::{header, HeaderMap, StatusCode},
    middleware::Next,
    response::Response,
};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::{app::AppState, error::AppError};

#[derive(Clone)]
pub struct AdminManager {
    username: Arc<str>,
    password: Arc<str>,
    session_ttl: Duration,
    sessions: Arc<RwLock<HashMap<String, Instant>>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionStatus {
    Valid,
    Expired,
    Invalid,
}

impl AdminManager {
    pub fn new(username: String, password: String, session_ttl: Duration) -> Self {
        let ttl = if session_ttl.is_zero() {
            Duration::from_secs(300)
        } else {
            session_ttl
        };

        Self {
            username: Arc::from(username.trim().to_string()),
            password: Arc::from(password),
            session_ttl: ttl,
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn verify_credentials(&self, username: &str, password: &str) -> bool {
        username == self.username.as_ref() && password == self.password.as_ref()
    }

    pub fn ttl_secs(&self) -> u64 {
        self.session_ttl.as_secs()
    }

    pub async fn issue_session(&self) -> String {
        self.prune_expired().await;
        let token = Uuid::new_v4().to_string();
        let expires_at = Instant::now() + self.session_ttl;
        self.sessions
            .write()
            .await
            .insert(token.clone(), expires_at);
        token
    }

    pub async fn validate_session(&self, token: &str) -> SessionStatus {
        let mut guard = self.sessions.write().await;
        let now = Instant::now();
        if let Some(expiry) = guard.get_mut(token) {
            if *expiry > now {
                *expiry = now + self.session_ttl;
                return SessionStatus::Valid;
            }
            // expired -> remove and signal Expired
            guard.remove(token);
            return SessionStatus::Expired;
        }
        SessionStatus::Invalid
    }

    pub async fn revoke_session(&self, token: &str) {
        self.sessions.write().await.remove(token);
    }

    async fn prune_expired(&self) {
        let now = Instant::now();
        self.sessions
            .write()
            .await
            .retain(|_, expiry| *expiry > now);
    }
}

pub async fn require_admin(
    axum::extract::State(state): axum::extract::State<AppState>,
    mut req: axum::http::Request<axum::body::Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let token = extract_bearer(req.headers()).or_else(|| {
        // Fallback: allow query param `token` (for SSE/EventSource which can't set headers)
        req.uri().query().and_then(|q| {
            let params = form_urlencoded::parse(q.as_bytes());
            for (k, v) in params {
                if k == "token" {
                    let s = v.trim().to_string();
                    if !s.is_empty() { return Some(s); }
                }
            }
            None
        })
    }).ok_or(StatusCode::UNAUTHORIZED)?;

    match state.admin.validate_session(&token).await {
        SessionStatus::Valid => {
            req.extensions_mut().insert(AdminIdentity {});
            Ok(next.run(req).await)
        }
        SessionStatus::Expired => {
            // 写入一条“管理员登出（会话过期）”事件，避免敏感信息泄露，不记录 token
            let pool = state.pool.clone();
            tokio::spawn(async move {
                let _ = crate::repo::events::upsert_event(
                    &pool,
                    &crate::repo::events::NewEvent {
                        level: "info".to_string(),
                        code: "ADMIN_LOGOUT".to_string(),
                        addition_info: Some("会话已过期，自动登出".to_string()),
                    },
                    0,
                ).await;
            });
            Err(StatusCode::UNAUTHORIZED)
        }
        SessionStatus::Invalid => Err(StatusCode::UNAUTHORIZED),
    }
}

fn extract_bearer(headers: &HeaderMap) -> Option<String> {
    let value = headers.get(header::AUTHORIZATION)?;
    let raw = value.to_str().ok()?;
    let token = raw
        .strip_prefix("Bearer ")
        .or_else(|| raw.strip_prefix("bearer "))?;
    if token.trim().is_empty() {
        None
    } else {
        Some(token.trim().to_string())
    }
}

#[derive(Clone, Copy, Debug)]
pub struct AdminIdentity;


pub fn invalid_credentials_error() -> AppError {
    AppError::Unauthorized("用户名或密码错误".to_string())
}
