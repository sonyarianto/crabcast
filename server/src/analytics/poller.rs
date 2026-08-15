//! Background analytics task (Phase 8): one tokio task that polls each
//! station's Icecast admin API for listener samples every minute, watches
//! media-disk free space every ten minutes, prunes data older than the
//! retention window every six hours, and raises/resolves alerts along the
//! way. Alerts raised by the supervisor (engine crash loops) and the engine
//! webhook (dead air) are resolved here once their conditions clear.

use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;

use sqlx::AnyPool;
use tokio::time::sleep;

use crate::analytics::{icecast, notify};
use crate::db::analytics;
use crate::db::stations;
use crate::stations::supervisor::Supervisor;

/// How often each station's Icecast admin API is polled (one sample per
/// station per tick; the roadmap calls for per-minute samples).
const LISTENER_INTERVAL: Duration = Duration::from_secs(60);
/// How often media-disk free space is checked.
const DISK_INTERVAL: Duration = Duration::from_secs(600);
/// How often old rows are purged.
const PURGE_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);
/// Free-space floor that triggers `disk_low`: below either of these.
const DISK_FREE_MIN_BYTES: u64 = 1 << 30; // 1 GiB
const DISK_FREE_MIN_PCT: f64 = 5.0;

pub struct AnalyticsPoller {
    pool: AnyPool,
    supervisor: Supervisor,
    media_root: PathBuf,
    retention_days: i64,
    alert_webhook: Option<String>,
    http: reqwest::Client,
}

impl AnalyticsPoller {
    pub fn new(pool: AnyPool, supervisor: Supervisor, media_root: PathBuf) -> Self {
        let retention_days = std::env::var("CRABCAST_RETENTION_DAYS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(30);
        let alert_webhook = std::env::var("CRABCAST_ALERT_WEBHOOK_URL").ok();
        Self {
            pool,
            supervisor,
            media_root,
            retention_days,
            alert_webhook,
            http: reqwest::Client::new(),
        }
    }

    /// Run forever. Each loop tick is one listener poll round plus any due
    /// disk/purge work; a slow tick (many stations or a hung Icecast) just
    /// delays the next round.
    pub async fn run(&self) {
        let mut next_disk = Instant::now();
        let mut next_purge = Instant::now();
        loop {
            self.tick_listeners().await;
            if next_disk <= Instant::now() {
                self.tick_disk().await;
                next_disk = Instant::now() + DISK_INTERVAL;
            }
            if next_purge <= Instant::now() {
                self.tick_purge().await;
                next_purge = Instant::now() + PURGE_INTERVAL;
            }
            sleep(LISTENER_INTERVAL).await;
        }
    }

    async fn tick_listeners(&self) {
        let stations = match stations::list(&self.pool).await {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("analytics: station list failed: {e}");
                return;
            }
        };
        for station in &stations {
            self.poll_station(station).await;
        }
    }

    async fn poll_station(&self, station: &stations::Station) {
        let stats = icecast::fetch_admin_stats(&self.http, station).await;
        match stats {
            Ok(s) => {
                if let Err(e) = analytics::insert_sample(
                    &self.pool,
                    &station.id,
                    s.listeners as i64,
                    s.listener_connections as i64,
                    true,
                )
                .await
                {
                    tracing::error!("analytics: sample insert failed: {e}");
                }
                if let Ok(resolved) =
                    analytics::resolve_open(&self.pool, Some(&station.id), "icecast_unreachable")
                        .await
                    && resolved > 0
                {
                    tracing::info!("station {}: icecast reachable again", station.id);
                }
            }
            Err(e) => {
                if let Err(e) = analytics::insert_sample(&self.pool, &station.id, 0, 0, false).await
                {
                    tracing::error!("analytics: sample insert failed: {e}");
                }
                match analytics::raise_alert(
                    &self.pool,
                    Some(&station.id),
                    "icecast_unreachable",
                    "warning",
                    "Icecast unreachable",
                    &e.to_string(),
                )
                .await
                {
                    Ok(Some(alert)) => {
                        tracing::warn!("station {}: alert icecast_unreachable: {e}", station.id);
                        notify(self.alert_webhook.as_deref(), "raised", &alert).await;
                    }
                    Ok(None) => {}
                    Err(e) => tracing::error!("analytics: raise_alert failed: {e}"),
                }
            }
        }

        // A crash-looping engine that has since stayed up resolves the
        // alert raised by the supervisor.
        let status = self.supervisor.status(&station.id).await;
        if status.uptime_seconds.unwrap_or(0) > 60
            && let Ok(resolved) =
                analytics::resolve_open(&self.pool, Some(&station.id), "engine_crash_loop").await
            && resolved > 0
        {
            tracing::info!(
                "station {}: engine stable, crash-loop alert cleared",
                station.id
            );
        }
    }

    async fn tick_disk(&self) {
        match disk_free_bytes(&self.media_root) {
            Ok((free, total)) => {
                let pct = if total > 0 {
                    100.0 * free as f64 / total as f64
                } else {
                    100.0
                };
                let low = free < DISK_FREE_MIN_BYTES || pct < DISK_FREE_MIN_PCT;
                let detail = format!(
                    "{:.1} GiB free of {:.1} GiB ({pct:.1}%)",
                    free as f64 / (1 << 30) as f64,
                    total as f64 / (1 << 30) as f64,
                );
                if low {
                    match analytics::raise_alert(
                        &self.pool,
                        None,
                        "disk_low",
                        "warning",
                        "Media storage low",
                        &detail,
                    )
                    .await
                    {
                        Ok(Some(alert)) => {
                            tracing::warn!("analytics: disk_low: {detail}");
                            notify(self.alert_webhook.as_deref(), "raised", &alert).await;
                        }
                        Ok(None) => {}
                        Err(e) => tracing::error!("analytics: raise_alert failed: {e}"),
                    }
                } else if let Ok(resolved) =
                    analytics::resolve_open(&self.pool, None, "disk_low").await
                    && resolved > 0
                {
                    tracing::info!("analytics: disk space recovered");
                }
            }
            Err(e) => tracing::error!("analytics: disk check on {:?} failed: {e}", self.media_root),
        }
    }

    async fn tick_purge(&self) {
        match analytics::purge(&self.pool, self.retention_days).await {
            Ok(n) if n > 0 => {
                tracing::info!(
                    "analytics: purged {n} rows older than {} days",
                    self.retention_days
                );
            }
            Ok(_) => {}
            Err(e) => tracing::error!("analytics: retention purge failed: {e}"),
        }
    }
}

/// Free and total bytes on the filesystem containing `path` (statvfs).
/// On non-unix platforms disk monitoring is skipped (free = MAX → never
/// raises `disk_low`).
#[cfg(unix)]
fn disk_free_bytes(path: &Path) -> std::io::Result<(u64, u64)> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    let c_path = CString::new(path.as_os_str().as_bytes())
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "nul in path"))?;
    let mut v: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(c_path.as_ptr(), &mut v) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    let free = v.f_bavail as u64 * v.f_frsize as u64;
    let total = v.f_blocks as u64 * v.f_frsize as u64;
    Ok((free, total))
}

#[cfg(not(unix))]
fn disk_free_bytes(_path: &Path) -> std::io::Result<(u64, u64)> {
    Ok((u64::MAX, u64::MAX))
}
