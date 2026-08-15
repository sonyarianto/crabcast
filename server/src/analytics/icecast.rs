//! Icecast admin API client (Phase 8): poll `GET /admin/stats` for each
//! station's mount and parse the current listener count plus Icecast's
//! cumulative connection counter (the unique-listener approximation).
//!
//! The stats XML varies across Icecast versions — the mount is either an
//! attribute (`<source mount="/radio">`) or a child element
//! (`<source><mount>/radio</mount></source>`) — so both forms are accepted.

use anyhow::bail;
use serde::Deserialize;

use crate::db::stations::Station;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IcecastStats {
    /// Current connected listeners on the mount.
    pub listeners: u64,
    /// Cumulative connections on the mount since it started; unique
    /// listeners over a window are approximated by its delta.
    pub listener_connections: u64,
}

/// Fetch + parse the listener stats for a station's mount.
pub async fn fetch_admin_stats(
    client: &reqwest::Client,
    station: &Station,
) -> anyhow::Result<IcecastStats> {
    let url = format!(
        "http://{}:{}/admin/stats",
        station.icecast_host, station.icecast_port
    );
    let res = client
        .get(&url)
        .basic_auth(
            &station.icecast_source_user,
            Some(&station.icecast_source_password),
        )
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await?;
    if !res.status().is_success() {
        bail!("icecast admin returned {}", res.status());
    }
    let xml = res.text().await?;
    parse_admin_stats(&xml, &station.icecast_mount)
}

#[derive(Debug, Deserialize)]
struct Icestats {
    #[serde(default)]
    source: Vec<Source>,
}

#[derive(Debug, Deserialize)]
struct Source {
    #[serde(rename = "@mount", default)]
    mount_attr: Option<String>,
    #[serde(rename = "mount", default)]
    mount_element: Option<String>,
    #[serde(default)]
    listeners: u64,
    #[serde(default)]
    listener_connections: u64,
}

/// Parse the `/admin/stats` XML and pull the stats for `mount`.
pub fn parse_admin_stats(xml: &str, mount: &str) -> anyhow::Result<IcecastStats> {
    let parsed: Icestats = quick_xml::de::from_str(xml)?;
    let target = mount.trim_start_matches('/');
    for src in &parsed.source {
        let m = src
            .mount_attr
            .as_deref()
            .or(src.mount_element.as_deref())
            .unwrap_or("")
            .trim_start_matches('/');
        if m == target {
            return Ok(IcecastStats {
                listeners: src.listeners,
                listener_connections: src.listener_connections,
            });
        }
    }
    bail!("mount {mount:?} not found in icecast admin stats")
}

#[cfg(test)]
mod tests {
    use super::*;

    const ATTR_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<icestats>
  <source mount="/radio">
    <listeners>3</listeners>
    <listener_connections>12</listener_connections>
    <bitrate>128</bitrate>
  </source>
  <source mount="/other">
    <listeners>9</listeners>
    <listener_connections>40</listener_connections>
  </source>
</icestats>"#;

    const ELEMENT_XML: &str = r#"<icestats>
  <source>
    <mount>/radio</mount>
    <listeners>5</listeners>
    <listener_connections>20</listener_connections>
  </source>
</icestats>"#;

    #[test]
    fn parses_attribute_mount_form() {
        let s = parse_admin_stats(ATTR_XML, "/radio").unwrap();
        assert_eq!(s.listeners, 3);
        assert_eq!(s.listener_connections, 12);
    }

    #[test]
    fn parses_element_mount_form() {
        let s = parse_admin_stats(ELEMENT_XML, "/radio").unwrap();
        assert_eq!(s.listeners, 5);
        assert_eq!(s.listener_connections, 20);
    }

    #[test]
    fn matches_exact_mount_ignoring_leading_slash() {
        let s = parse_admin_stats(ATTR_XML, "radio").unwrap();
        assert_eq!(s.listeners, 3);
        assert!(parse_admin_stats(ATTR_XML, "/nope").is_err());
    }
}
