// no-op: remove unused imports

use axum::{
    body::Body,
    http::{HeaderValue, Request},
    middleware::Next,
    response::Response,
};
use uuid::Uuid;

use crate::{app::AppState, ops::events as ops_events};

pub async fn assign_trace_id(mut req: Request<Body>, next: Next) -> Response {
    let trace_id = Uuid::new_v4().to_string();
    req.extensions_mut().insert(trace_id.clone());
    let mut res = next.run(req).await;
    res.headers_mut()
        .insert("X-Trace-Id", HeaderValue::from_str(&trace_id).unwrap_or(HeaderValue::from_static("invalid")));
    res
}

use axum::extract::State;

pub async fn report_internal_errors(
    State(state): State<AppState>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let path = req.uri().path().to_string();
    let method = req.method().to_string();
    let trace_id = req
        .extensions()
        .get::<String>()
        .cloned()
        .unwrap_or_else(|| "".to_string());

    let res = next.run(req).await;
    if res.status().as_u16() >= 500 {
        // best-effort emit one error event per 500
        let _ = ops_events::emit(
            &state.pool,
            &state.events,
            ops_events::EmitEvent {
                level: "error".to_string(),
                code: "INTERNAL_SERVER_ERROR".to_string(),
                title: "服务内部错误".to_string(),
                message: format!("500 on {} {}", method, path),
                attrs: serde_json::json!({
                    "method": method,
                    "path": path,
                    "trace_id": trace_id,
                }),
                source: "api".to_string(),
                dedupe_key: Some(format!("route:{}", path)),
            },
        )
        .await;
    }
    res
}
