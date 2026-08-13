//! Station supervisor: one supervised `crabsoup` process per station.
//!
//! Lifecycle: generate `crabsoup.lua` from the DB model → `crabsoup --check`
//! → write under the station's config dir → spawn with logs captured to a
//! file → restart with exponential backoff on crash. Config applies are
//! atomic: write the new Lua to a temp file, `--check` it, swap, then
//! restart the process (the child is killed so the watchdog respawns with
//! the new script).

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use serde::Serialize;
use tokio::process::{Child, Command};
use tokio::sync::Mutex;
use tokio::time::sleep;

use crate::db::stations::Station;
use crate::lua;

/// Max backoff between crash restarts.
const MAX_BACKOFF: Duration = Duration::from_secs(30);
/// Initial backoff before the first respawn.
const INITIAL_BACKOFF: Duration = Duration::from_millis(500);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcessState {
    Running,
    Stopped,
    Failed,
}

#[derive(Debug, Serialize)]
pub struct StationStatus {
    pub state: ProcessState,
    pub pid: Option<u32>,
    pub uptime_seconds: Option<u64>,
    pub restarts: u64,
    pub last_error: Option<String>,
}

#[derive(Clone)]
struct ProcessHandle {
    pid: u32,
    started_at: Instant,
    stop: Arc<AtomicBool>,
}

#[derive(Default)]
struct Registry {
    /// Metadata per station; present while a child may be alive.
    processes: HashMap<String, ProcessHandle>,
    /// The live `Child` handles, owned by the registry until a watchdog
    /// takes them to `wait()` (or `stop()` takes them to `kill()`).
    children: HashMap<String, Child>,
    restarts: HashMap<String, u64>,
    last_error: HashMap<String, Option<String>>,
}

#[derive(Clone)]
pub struct Supervisor {
    /// Directory holding per-station `configs/<id>/crabsoup.lua` and
    /// `logs/<id>.log`.
    base_dir: PathBuf,
    crabsoup_bin: PathBuf,
    webhook_url: Option<String>,
    pool: sqlx::SqlitePool,
    registry: Arc<Mutex<Registry>>,
}

impl Supervisor {
    pub fn new(base_dir: impl Into<PathBuf>, pool: sqlx::SqlitePool) -> Self {
        let base_dir = base_dir.into();
        let crabsoup_bin = std::env::var("CRABSOUP_BIN")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("crabsoup"));
        let webhook_url = std::env::var("CRABCAST_WEBHOOK_URL").ok();
        Self {
            base_dir,
            crabsoup_bin,
            webhook_url,
            pool,
            registry: Arc::new(Mutex::new(Registry::default())),
        }
    }

    fn config_path(&self, id: &str) -> PathBuf {
        self.base_dir.join("configs").join(id).join("crabsoup.lua")
    }

    fn log_path(&self, id: &str) -> PathBuf {
        self.base_dir.join("logs").join(format!("{id}.log"))
    }

    /// Render + `--check` + atomically write the station's `crabsoup.lua`.
    async fn write_config(&self, station: &Station) -> anyhow::Result<()> {
        let webhook = self
            .webhook_url
            .as_deref()
            .unwrap_or("http://localhost:8080/api/webhooks/track");
        let script = lua::render(station, webhook);

        // Validate before touching anything.
        lua::validate(&script, &self.crabsoup_bin).map_err(|e| {
            tracing::error!("station {}: crabsoup --check failed: {e}", station.id);
            anyhow::anyhow!("config rejected by crabsoup --check: {e}")
        })?;

        let config_path = self.config_path(&station.id);
        if let Some(parent) = config_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let tmp = config_path.with_extension("lua.tmp");
        tokio::fs::write(&tmp, &script).await?;
        tokio::fs::rename(&tmp, &config_path).await?;
        Ok(())
    }

    /// Start (or restart) the engine for a station: render + check + write
    /// the script, then spawn the process.
    pub async fn apply(&self, station: &Station) -> anyhow::Result<()> {
        // Atomic config swap: kill the running child (the watchdog exits
        // on the stop flag), then spawn fresh with the new script.
        self.stop(&station.id).await?;
        self.spawn(station).await?;
        Ok(())
    }

    /// Spawn the engine for a station, storing the handle and starting a
    /// watchdog that takes the `Child` and restarts on exit. The config is
    /// (re)written first so boot and respawn never see a missing script.
    async fn spawn(&self, station: &Station) -> anyhow::Result<()> {
        {
            let reg = self.registry.lock().await;
            if reg.processes.contains_key(&station.id) {
                return Ok(());
            }
        }

        self.write_config(station).await?;
        let (pid, child) = self.spawn_process(station)?;

        let handle = ProcessHandle {
            pid,
            started_at: Instant::now(),
            stop: Arc::new(AtomicBool::new(false)),
        };
        {
            let mut reg = self.registry.lock().await;
            reg.processes.insert(station.id.clone(), handle.clone());
            reg.children.insert(station.id.clone(), child);
            reg.last_error.insert(station.id.clone(), None);
        }

        let self2 = self.clone();
        let id = station.id.clone();
        tokio::spawn(async move {
            let mut backoff = INITIAL_BACKOFF;
            loop {
                // Take the child out of the registry and wait on it. If
                // `stop()` already took it, the process is gone and this
                // watchdog is done.
                let mut child = {
                    let mut reg = self2.registry.lock().await;
                    match reg.children.remove(&id) {
                        Some(c) => c,
                        None => return,
                    }
                };
                let status = child.wait().await;
                if handle.stop.load(Ordering::SeqCst) {
                    tracing::info!("station {id}: stopped by supervisor");
                    return;
                }
                let code = match status {
                    Ok(s) => s
                        .code()
                        .map(|c| c.to_string())
                        .unwrap_or_else(|| "signal".into()),
                    Err(e) => format!("wait error: {e}"),
                };
                tracing::warn!(
                    "station {id}: crabsoup (pid {}) exited ({code}); restarting in {backoff:?}",
                    handle.pid
                );

                {
                    let mut reg = self2.registry.lock().await;
                    reg.processes.remove(&id);
                    let restarts = reg.restarts.entry(id.clone()).or_insert(0);
                    *restarts += 1;
                    reg.last_error
                        .insert(id.clone(), Some(format!("exited: {code}")));
                }

                sleep(backoff).await;
                backoff = (backoff * 2).min(MAX_BACKOFF);

                // Re-read the station from the DB so the respawn uses the
                // latest config (the config file may have been swapped).
                let station = match crate::db::stations::get(&self2.pool, &id).await {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::error!("station {id}: reload for respawn failed: {e}");
                        continue;
                    }
                };
                match self2.respawn(&station).await {
                    Ok(()) => backoff = INITIAL_BACKOFF,
                    Err(e) => {
                        tracing::error!("station {id}: respawn failed: {e}");
                        let mut reg = self2.registry.lock().await;
                        reg.last_error.insert(id.clone(), Some(e.to_string()));
                        drop(reg);
                    }
                }
            }
        });

        Ok(())
    }

    /// Re-launch `crabsoup` for a station that just crashed. Replaces the
    /// handle in the registry but does *not* start a new watchdog (the
    /// calling watchdog keeps looping).
    async fn respawn(&self, station: &Station) -> anyhow::Result<()> {
        self.write_config(station).await?;
        let (pid, child) = self.spawn_process(station)?;
        let handle = ProcessHandle {
            pid,
            started_at: Instant::now(),
            stop: Arc::new(AtomicBool::new(false)),
        };
        let mut reg = self.registry.lock().await;
        reg.processes.insert(station.id.clone(), handle);
        reg.children.insert(station.id.clone(), child);
        reg.last_error.insert(station.id.clone(), None);
        Ok(())
    }

    /// Launch the `crabsoup` process itself: config path, log capture,
    /// stdin closed. Returns the pid and the `Child`.
    fn spawn_process(&self, station: &Station) -> anyhow::Result<(u32, Child)> {
        let config_path = self.config_path(&station.id);
        let log_path = self.log_path(&station.id);
        if let Some(parent) = log_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let log_file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)?;

        let child = Command::new(&self.crabsoup_bin)
            .arg("-c")
            .arg(&config_path)
            .stdout(Stdio::from(log_file.try_clone()?))
            .stderr(Stdio::from(log_file))
            .stdin(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| anyhow::anyhow!("failed to spawn crabsoup: {e}"))?;

        let pid = child.id().unwrap_or(0);
        tracing::info!(
            "station {}: spawned crabsoup pid {pid} (config {})",
            station.id,
            config_path.display()
        );
        Ok((pid, child))
    }

    /// Start every station from the DB (boot).
    pub async fn start_all(&self, stations: &[Station]) {
        for station in stations {
            if let Err(e) = self.spawn(station).await {
                tracing::error!("station {}: failed to start at boot: {e}", station.id);
                let mut reg = self.registry.lock().await;
                reg.last_error
                    .insert(station.id.clone(), Some(e.to_string()));
            }
        }
    }

    /// Stop the engine for a station (no respawn).
    pub async fn stop(&self, id: &str) -> anyhow::Result<()> {
        let mut reg = self.registry.lock().await;
        if let Some(handle) = reg.processes.remove(id) {
            handle.stop.store(true, Ordering::SeqCst);
            if let Some(mut child) = reg.children.remove(id) {
                let _ = child.kill().await;
                let _ = child.wait().await;
            }
        }
        Ok(())
    }

    /// Current status of one station.
    pub async fn status(&self, id: &str) -> StationStatus {
        let reg = self.registry.lock().await;
        let restarts = reg.restarts.get(id).copied().unwrap_or(0);
        let last_error = reg.last_error.get(id).cloned().flatten();
        match reg.processes.get(id) {
            Some(handle) => StationStatus {
                state: ProcessState::Running,
                pid: Some(handle.pid),
                uptime_seconds: Some(handle.started_at.elapsed().as_secs()),
                restarts,
                last_error,
            },
            None => StationStatus {
                state: if last_error.is_some() {
                    ProcessState::Failed
                } else {
                    ProcessState::Stopped
                },
                pid: None,
                uptime_seconds: None,
                restarts,
                last_error,
            },
        }
    }

    /// Kill all children (shutdown).
    pub async fn shutdown(&self) {
        let mut reg = self.registry.lock().await;
        for (_, handle) in reg.processes.drain() {
            handle.stop.store(true, Ordering::SeqCst);
        }
        for (_, mut child) in reg.children.drain() {
            let _ = child.kill().await;
            let _ = child.wait().await;
        }
    }
}
