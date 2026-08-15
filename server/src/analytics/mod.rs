//! Analytics & monitoring (Phase 8): Icecast listener polling, the alert
//! feed, retention cleanup, and outbound alert webhooks.
//!
//! The poller runs as one background task (spawned from `main`): every
//! minute it polls each station's Icecast admin API for listener counts,
//! every ten minutes it checks media-disk free space, and every six hours
//! it prunes data older than the retention window. Alerts raised here (and
//! from the supervisor) are deduplicated in the DB and optionally posted to
//! a webhook URL.

pub mod icecast;
pub mod poller;

use crate::db::analytics::Alert;

/// POST an alert lifecycle event to the configured webhook, if any.
/// Best-effort: failures are logged, never fatal.
pub async fn notify(webhook_url: Option<&str>, event: &str, alert: &Alert) {
    let Some(url) = webhook_url else { return };
    let body = serde_json::json!({
        "event": event,
        "alert": alert,
    });
    let client = reqwest::Client::new();
    if let Err(e) = client
        .post(url)
        .json(&body)
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
    {
        tracing::warn!("alert webhook {url} failed: {e}");
    }
}
