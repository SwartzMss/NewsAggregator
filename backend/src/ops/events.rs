use std::time::Duration;

use axum::response::sse::{Event as SseEvent, KeepAlive, Sse};
use futures::{Stream, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

use crate::repo::events as repo_events;

#[derive(Clone)]
pub struct EventsHub {
    sender: broadcast::Sender<repo_events::EventRecord>,
}

impl EventsHub {
    pub fn new(buffer: usize) -> Self {
        let (tx, _rx) = broadcast::channel(buffer);
        Self { sender: tx }
    }

    #[allow(dead_code)]
    pub fn broadcast(&self, ev: repo_events::EventRecord) {
        let _ = self.sender.send(ev);
    }

    pub fn stream(&self) -> impl Stream<Item = Result<SseEvent, std::convert::Infallible>> {
        let rx = self.sender.subscribe();
        tokio_stream::wrappers::BroadcastStream::new(rx).filter_map(|item| async move {
            match item {
                Ok(ev) => {
                    let json = serde_json::to_string(&ev).unwrap_or_else(|_| "{}".to_string());
                    Some(Ok(SseEvent::default().event("alert").data(json)))
                }
                Err(_e) => None,
            }
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EmitEvent {
    pub level: String,
    pub code: String,
    pub source_domain: Option<String>,
}

#[allow(dead_code)]
pub async fn emit(
    pool: &sqlx::PgPool,
    hub: &EventsHub,
    payload: EmitEvent,
) -> anyhow::Result<repo_events::EventRecord> {
    let stored = repo_events::upsert_event(
        pool,
        &repo_events::NewEvent {
            level: payload.level,
            code: payload.code,
            source_domain: payload.source_domain,
        },
        300,
    )
    .await?;
    hub.broadcast(stored.clone());
    Ok(stored)
}

pub fn sse_response(hub: &EventsHub) -> Sse<impl Stream<Item = Result<SseEvent, std::convert::Infallible>>> {
    Sse::new(hub.stream()).keep_alive(KeepAlive::new().interval(Duration::from_secs(20)))
}
