// SPDX-License-Identifier: Apache-2.0
// Modified from the original Moor project for this Web/Docker distribution; see NOTICE.

use crate::core::http::AppState;
use axum::{
    extract::State,
    response::sse::{Event, KeepAlive, Sse},
    routing::get,
    Router,
};
use futures::stream::Stream;
use std::{convert::Infallible, sync::Arc};
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/api/events", get(events))
}

async fn events(
    State(state): State<Arc<AppState>>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let receiver = state.event_bus.subscribe();
    let stream = BroadcastStream::new(receiver);

    let initial = futures::stream::once(async {
        Ok(Event::default()
            .event("connected")
            .data(serde_json::json!({"timestamp": chrono::Utc::now().to_rfc3339()}).to_string()))
    });

    let event_stream = stream.filter_map(|result| match result {
        Ok(evt) => {
            let event = Event::default()
                .event(evt.name())
                .data(evt.payload().to_string());
            Some(Ok(event))
        }
        Err(_) => None,
    });

    let combined = initial.chain(event_stream);

    Sse::new(combined).keep_alive(
        KeepAlive::new()
            .interval(std::time::Duration::from_secs(30))
            .text("heartbeat"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::SystemTime;

    fn temp_data_dir(test_name: &str) -> std::path::PathBuf {
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("system time is before unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("moor-events-route-{test_name}-{timestamp}"))
    }

    fn test_state(data_dir: std::path::PathBuf) -> Arc<AppState> {
        AppState::for_test(&data_dir)
    }

    #[tokio::test]
    async fn events_stream_constructs_keep_alive_without_panicking() {
        let data_dir = temp_data_dir("keep-alive");
        let state = test_state(data_dir.clone());

        let _stream = events(State(state)).await;

        let _ = std::fs::remove_dir_all(data_dir);
    }
}
