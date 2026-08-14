//! Crabsoup HTTP control client (`server.telnet({http_port = N})`).
//!
//! Implements the engine's control surface documented in
//! `../crabsoup/website/guide/control-port.md`: `GET /status`, `/uptime`,
//! `/queue`, `/jingles`, and `POST /cmd`. Every reply uses the
//! `{"ok": true, ...}` envelope.

use serde::Deserialize;

/// Base URL of one station's Crabsoup control HTTP port
/// (e.g. `http://127.0.0.1:9234`).
#[derive(Clone, Debug)]
pub struct ControlClient {
    base: String,
    http: reqwest::Client,
}

/// Generic envelope; command replies carry extra fields alongside `ok`.
#[derive(Debug, Deserialize)]
pub struct Reply {
    pub ok: bool,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Status {
    #[serde(default)]
    pub playing: String,
    #[serde(default)]
    pub uptime_seconds: u64,
    /// True while a live DJ holds the harbor (playlist ducked).
    #[serde(default)]
    pub harbor_connected: bool,
}

impl ControlClient {
    pub fn new(base: impl Into<String>) -> Self {
        Self {
            base: base.into(),
            http: reqwest::Client::new(),
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base.trim_end_matches('/'), path)
    }

    pub async fn status(&self) -> anyhow::Result<Status> {
        Ok(self
            .http
            .get(self.url("/status"))
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?)
    }

    // Only the tests exercise these endpoints today; the dashboard
    // phase wires them into the UI.
    #[allow(dead_code)]
    pub async fn uptime(&self) -> anyhow::Result<u64> {
        #[derive(Deserialize)]
        struct Body {
            uptime_seconds: u64,
        }
        Ok(self
            .http
            .get(self.url("/uptime"))
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await?
            .error_for_status()?
            .json::<Body>()
            .await?
            .uptime_seconds)
    }

    #[allow(dead_code)]
    pub async fn queue(&self) -> anyhow::Result<Vec<String>> {
        #[derive(Deserialize)]
        struct Body {
            #[serde(default)]
            queue: Vec<String>,
        }
        Ok(self
            .http
            .get(self.url("/queue"))
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await?
            .error_for_status()?
            .json::<Body>()
            .await?
            .queue)
    }

    #[allow(dead_code)]
    pub async fn jingles(&self) -> anyhow::Result<Vec<String>> {
        #[derive(Deserialize)]
        struct Body {
            #[serde(default)]
            jingles: Vec<String>,
        }
        Ok(self
            .http
            .get(self.url("/jingles"))
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await?
            .error_for_status()?
            .json::<Body>()
            .await?
            .jingles)
    }

    /// Send any control command (`skip`, `queue.push <uri>`,
    /// `jingles.play`, ...). Engine errors come back as HTTP 400 with the
    /// `{"ok": false, "error": ...}` body.
    pub async fn cmd(&self, command: &str) -> anyhow::Result<Reply> {
        let res = self
            .http
            .post(self.url("/cmd"))
            .json(&serde_json::json!({ "command": command }))
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await?;
        let body = res.json::<Reply>().await?;
        if !body.ok {
            anyhow::bail!(
                "engine rejected command {command:?}: {}",
                body.error.as_deref().unwrap_or("unknown error")
            );
        }
        Ok(body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::routing::get;
    use axum::{Json, Router};
    use serde_json::json;

    /// Mock engine control server with the real envelope.
    async fn mock_engine() -> (ControlClient, String) {
        let app = Router::new()
            .route(
                "/status",
                get(|| async {
                    Json(json!({"ok": true, "playing": "Song A", "uptime_seconds": 42}))
                }),
            )
            .route(
                "/uptime",
                get(|| async { Json(json!({"ok": true, "uptime_seconds": 42})) }),
            )
            .route(
                "/queue",
                get(|| async { Json(json!({"ok": true, "queue": ["a.mp3", "b.mp3"]})) }),
            )
            .route(
                "/jingles",
                get(|| async { Json(json!({"ok": true, "jingles": ["j.mp3"]})) }),
            )
            .route(
                "/cmd",
                axum::routing::post(|body: Json<serde_json::Value>| async move {
                    match body.get("command").and_then(|c| c.as_str()) {
                        Some("skip") => Json(json!({"ok": true})),
                        _ => Json(json!({"ok": false, "error": "unknown command"})),
                    }
                }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let client = ControlClient::new(format!("http://{addr}"));
        (client, addr.to_string())
    }

    #[tokio::test]
    async fn status_parses_envelope() {
        let (client, _) = mock_engine().await;
        let s = client.status().await.unwrap();
        assert_eq!(s.playing, "Song A");
        assert_eq!(s.uptime_seconds, 42);
    }

    #[tokio::test]
    async fn uptime_queue_jingles_parse() {
        let (client, _) = mock_engine().await;
        assert_eq!(client.uptime().await.unwrap(), 42);
        assert_eq!(client.queue().await.unwrap(), vec!["a.mp3", "b.mp3"]);
        assert_eq!(client.jingles().await.unwrap(), vec!["j.mp3"]);
    }

    #[tokio::test]
    async fn cmd_ok_and_rejection() {
        let (client, _) = mock_engine().await;
        let reply = client.cmd("skip").await.unwrap();
        assert!(reply.ok);

        let err = client.cmd("bogus").await.unwrap_err();
        assert!(err.to_string().contains("unknown command"));
    }
}
