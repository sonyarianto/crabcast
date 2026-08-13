//! SSE hub: per-station event broadcast channels.
//!
//! The engine pushes track changes to the webhook receiver; the receiver
//! records them in the DB and publishes them here. Dashboard pages
//! subscribe via `GET /api/stations/:id/events`.

use std::collections::HashMap;

use serde::Serialize;
use tokio::sync::Mutex;
use tokio::sync::broadcast;

#[derive(Clone, Debug, Serialize)]
pub struct TrackEvent {
    pub title: String,
    pub started_at: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct StatusEvent {
    pub state: String,
    pub playing: Option<String>,
    pub uptime_seconds: Option<u64>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", content = "data")]
pub enum StationEvent {
    Track(TrackEvent),
    Status(StatusEvent),
}

#[derive(Default)]
pub struct SseHub {
    channels: Mutex<HashMap<String, broadcast::Sender<StationEvent>>>,
}

impl SseHub {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn subscribe(&self, station_id: &str) -> broadcast::Receiver<StationEvent> {
        let mut channels = self.channels.lock().await;
        let tx = channels
            .entry(station_id.to_string())
            .or_insert_with(|| broadcast::channel(64).0)
            .clone();
        tx.subscribe()
    }

    pub async fn publish(&self, station_id: &str, event: StationEvent) {
        let channels = self.channels.lock().await;
        let Some(tx) = channels.get(station_id) else {
            return;
        };
        // Drop slow consumers rather than stalling the hub.
        let _ = tx.send(event);
    }
}

/// Serialize a StationEvent as the SSE `data:` payload (JSON).
pub fn sse_frame(event: &StationEvent) -> String {
    serde_json::to_string(event).unwrap_or_default()
}
