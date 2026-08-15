//! SSE hub: per-station event broadcast channels.
//!
//! The engine pushes track changes to the webhook receiver; the receiver
//! records them in the DB and publishes them here. Dashboard pages
//! subscribe via `GET /api/stations/:id/events`.
//!
//! Two backends:
//! - **local** (default): in-process broadcast channels — each API host
//!   only fans out events it handled itself.
//! - **Redis pub/sub** (`CRABCAST_REDIS_URL`, e.g. `redis://host:6379`):
//!   events are published to a `crabcast:station:{id}:events` channel and
//!   every API host subscribes to it, so N hosts share one bus (Phase 9
//!   horizontal scale). When Redis is configured, publish and subscribe
//!   both go through Redis only.

use std::collections::HashMap;
use std::pin::Pin;

use serde::Serialize;
use tokio::sync::Mutex;
use tokio::sync::broadcast;
use tokio_stream::Stream;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::{BroadcastStream, ReceiverStream};

#[derive(Clone, Debug, Serialize, serde::Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum StationEvent {
    Track(TrackEvent),
    Status(StatusEvent),
}

#[derive(Clone, Debug, Serialize, serde::Deserialize)]
pub struct TrackEvent {
    pub title: String,
    pub started_at: String,
}

#[derive(Clone, Debug, Serialize, serde::Deserialize)]
pub struct StatusEvent {
    pub state: String,
    pub playing: Option<String>,
    pub uptime_seconds: Option<u64>,
}

/// A live stream of station events (broadcast lag is dropped).
pub type StationStream = Pin<Box<dyn Stream<Item = StationEvent> + Send>>;

pub struct SseHub {
    channels: Mutex<HashMap<String, broadcast::Sender<StationEvent>>>,
    redis: Option<RedisBus>,
}

/// Lazy Redis connection: the client is built at startup (cheap), the
/// `ConnectionManager` (a pooled TCP connection) is opened on first use so
/// a missing Redis never blocks boot.
struct RedisBus {
    client: redis::Client,
    manager: Mutex<Option<redis::aio::ConnectionManager>>,
}

impl RedisBus {
    fn channel(station_id: &str) -> String {
        format!("crabcast:station:{station_id}:events")
    }

    async fn manager(&self) -> redis::RedisResult<redis::aio::ConnectionManager> {
        let mut guard = self.manager.lock().await;
        if let Some(m) = guard.as_ref() {
            return Ok(m.clone());
        }
        let m = self.client.get_connection_manager().await?;
        *guard = Some(m.clone());
        Ok(m)
    }

    async fn subscribe(&self, station_id: &str) -> redis::RedisResult<StationStream> {
        // A dedicated pubsub connection per subscriber; the client handles
        // reconnect. `on_message()` borrows the pubsub, so both are moved
        // into the forwarding task to get an owned 'static stream.
        let mut pubsub = self.client.get_async_pubsub().await?;
        pubsub.subscribe(Self::channel(station_id)).await?;
        let (tx, rx) = tokio::sync::mpsc::channel(64);
        tokio::spawn(async move {
            let mut stream = pubsub.on_message();
            while let Some(msg) = stream.next().await {
                let Ok(payload) = msg.get_payload::<String>() else {
                    continue;
                };
                let Ok(event) = serde_json::from_str::<StationEvent>(&payload) else {
                    continue;
                };
                if tx.send(event).await.is_err() {
                    break;
                }
            }
        });
        Ok(Box::pin(ReceiverStream::new(rx)))
    }

    async fn publish(&self, station_id: &str, event: &StationEvent) -> redis::RedisResult<()> {
        use redis::AsyncCommands;
        let mut manager = self.manager().await?;
        let payload = serde_json::to_string(event).unwrap_or_default();
        AsyncCommands::publish(&mut manager, Self::channel(station_id), payload).await
    }
}

impl SseHub {
    pub fn new() -> Self {
        let redis = match std::env::var("CRABCAST_REDIS_URL") {
            Ok(url) if !url.trim().is_empty() => match redis::Client::open(url.trim()) {
                Ok(client) => {
                    tracing::info!("SSE hub: Redis pub/sub enabled");
                    Some(RedisBus {
                        client,
                        manager: Mutex::new(None),
                    })
                }
                Err(e) => {
                    tracing::warn!(
                        "CRABCAST_REDIS_URL set but invalid ({e}); using the local in-process hub"
                    );
                    None
                }
            },
            _ => None,
        };
        Self {
            channels: Mutex::new(HashMap::new()),
            redis,
        }
    }

    pub fn redis_enabled(&self) -> bool {
        self.redis.is_some()
    }

    pub async fn subscribe(&self, station_id: &str) -> StationStream {
        if let Some(bus) = &self.redis {
            match bus.subscribe(station_id).await {
                Ok(stream) => return stream,
                Err(e) => {
                    tracing::error!("redis subscribe failed ({e}); falling back to the local hub");
                }
            }
        }
        let mut channels = self.channels.lock().await;
        let tx = channels
            .entry(station_id.to_string())
            .or_insert_with(|| broadcast::channel(64).0)
            .clone();
        Box::pin(BroadcastStream::new(tx.subscribe()).filter_map(|r| r.ok()))
    }

    pub async fn publish(&self, station_id: &str, event: StationEvent) {
        if let Some(bus) = &self.redis {
            if let Err(e) = bus.publish(station_id, &event).await {
                tracing::error!("redis publish failed: {e}");
            }
            return;
        }
        let channels = self.channels.lock().await;
        let Some(tx) = channels.get(station_id) else {
            return;
        };
        // Drop slow consumers rather than stalling the hub.
        let _ = tx.send(event);
    }
}

impl Default for SseHub {
    fn default() -> Self {
        Self::new()
    }
}

/// Serialize a StationEvent as the SSE `data:` payload (JSON).
pub fn sse_frame(event: &StationEvent) -> String {
    serde_json::to_string(event).unwrap_or_default()
}
