use std::time::Duration;

use axum::response::sse::{Event as SseEvent, KeepAlive, Sse};
use futures::{Stream, StreamExt};
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


pub fn sse_response(hub: &EventsHub) -> Sse<impl Stream<Item = Result<SseEvent, std::convert::Infallible>>> {
    Sse::new(hub.stream()).keep_alive(KeepAlive::new().interval(Duration::from_secs(20)))
}
