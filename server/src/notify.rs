//! On-air notifications (Phase 11): posts a station event to every
//! webhook subscribed to it. The payload carries both `text` (Slack) and
//! `content` (Discord) fields — each service ignores the other's key, so
//! one payload works for both. Dispatch is fire-and-forget: a slow or
//! failing webhook must never block the station lifecycle.
//!
//! The alert webhook (`CRABCAST_ALERT_WEBHOOK_URL`) remains the separate,
//! env-configured crash-alert channel from Phase 4.

use sqlx::AnyPool;

use crate::db::notification_webhooks;

/// Human-readable message per event.
fn message_for(event: &str, station_name: &str) -> String {
    match event {
        "started" => format!("🟢 **{station_name}** is now on air"),
        "stopped" => format!("⏹️ **{station_name}** went off air"),
        "crashed" => format!("⚠️ **{station_name}** crashed and is restarting"),
        "blank" => format!("🔇 **{station_name}** has dead air"),
        _ => format!("**{station_name}**: {event}"),
    }
}

/// Fire an event for a station: look up subscribed webhooks and POST to
/// each with a 5s timeout. Errors are logged, never propagated.
pub async fn station_event(pool: &AnyPool, station_id: &str, event: &str) {
    let station_name = match crate::db::stations::get(pool, station_id).await {
        Ok(s) => s.name,
        Err(_) => station_id.to_string(),
    };
    let webhooks = match notification_webhooks::for_event(pool, station_id, event).await {
        Ok(w) => w,
        Err(e) => {
            tracing::warn!("notify: load webhooks for {station_id}/{event}: {e}");
            return;
        }
    };
    if webhooks.is_empty() {
        return;
    }
    let message = message_for(event, &station_name);
    let body = serde_json::json!({
        // `event` is ignored by Slack/Discord but helps webhook consumers.
        "event": event,
        "text": message,
        "content": message,
    });
    let client = reqwest::Client::new();
    for wh in webhooks {
        if let Err(e) = client
            .post(&wh.url)
            .json(&body)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
        {
            tracing::warn!(
                "notify: webhook {} failed for {station_id}/{event}: {e}",
                wh.url
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_are_per_event() {
        assert!(message_for("started", "Radio A").contains("Radio A"));
        assert!(message_for("crashed", "Radio A").contains("crashed"));
        assert!(message_for("blank", "Radio A").contains("dead air"));
        assert!(message_for("stopped", "Radio A").contains("off air"));
    }

    #[test]
    fn payload_carries_slack_and_discord_fields() {
        let message = message_for("started", "Radio A");
        let body = serde_json::json!({ "text": message, "content": message });
        assert_eq!(body["text"], body["content"]);
        assert!(body["text"].as_str().unwrap().contains("Radio A"));
    }
}
