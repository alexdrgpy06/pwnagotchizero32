//! Attack engine — spawns and supervises AngryOxide as the WiFi attack and
//! capture tool, replacing the old per-AP aireplay-ng calls.
//!
//! AngryOxide owns the monitor interface once started: it does its own
//! channel hopping, scanning, targeting, and multi-vector attacking
//! (deauth, PMKID, anon-reassoc, CSA, disassoc, rogue M2) in one long-lived
//! process, and writes ready-to-use .hc22000/.pcapng files as it captures.
//! We just keep it alive and let `CaptureManager` pick up its output.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use tokio::process::{Child, Command};

use crate::config::Config;

/// Per-attack-type toggles and aggressiveness, adjustable live from the web
/// dashboard — AngryOxide already exposes exactly these as CLI flags
/// (--disable-deauth/--disable-pmkid/etc, -r for rate), so this just mirrors
/// them instead of inventing a new attack-control scheme.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttackSettings {
    /// 1-3, passed straight through to AngryOxide's -r (3 = most aggressive).
    pub rate: u32,
    pub deauth: bool,
    pub pmkid: bool,
    pub anon: bool,
    pub csa: bool,
    pub disassoc: bool,
    pub roguem2: bool,
}

/// Cheaply-cloneable handle shared between the web server (writes new
/// settings) and the attack engine (reads them on every spawn).
pub type SharedAttackSettings = Arc<RwLock<AttackSettings>>;

impl AttackSettings {
    fn from_config(config: &Config) -> Self {
        Self {
            rate: config.oxigotchi.attack_rate_limit.clamp(1, 3),
            deauth: true,
            pmkid: true,
            anon: true,
            csa: true,
            disassoc: true,
            roguem2: true,
        }
    }
}

pub struct AttackEngine {
    config: Arc<Config>,
    child: Option<Child>,
    whitelist_file: PathBuf,
    started_at: Option<Instant>,
    restart_count: u32,
    settings: SharedAttackSettings,
    /// Set by the web dashboard when settings change; ensure_running() kills
    /// and respawns AngryOxide on the next check so the new flags actually
    /// take effect, instead of waiting for it to crash on its own.
    restart_requested: Arc<AtomicBool>,
}

impl AttackEngine {
    pub async fn new(config: &Arc<Config>) -> Result<Self> {
        let whitelist_file = PathBuf::from(&config.bettercap.handshakes).join("angryoxide-whitelist.txt");
        Self::write_whitelist(&whitelist_file, &config.main.whitelist).await?;

        Ok(Self {
            settings: Arc::new(RwLock::new(AttackSettings::from_config(config))),
            config: config.clone(),
            child: None,
            whitelist_file,
            started_at: None,
            restart_count: 0,
            restart_requested: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Handle the web dashboard uses to read/write live attack settings.
    pub fn settings_handle(&self) -> SharedAttackSettings {
        self.settings.clone()
    }

    /// Handle the web dashboard uses to force AngryOxide to restart with
    /// whatever settings are current, instead of waiting for it to crash.
    pub fn restart_flag(&self) -> Arc<AtomicBool> {
        self.restart_requested.clone()
    }

    async fn write_whitelist(path: &PathBuf, entries: &[String]) -> Result<()> {
        if let Some(parent) = path.parent() {
            let _ = tokio::fs::create_dir_all(parent).await;
        }
        tokio::fs::write(path, entries.join("\n")).await?;
        Ok(())
    }

    /// Make sure AngryOxide is running against the given monitor interface.
    /// Cheap no-op if already running. If it has died, respawns with
    /// exponential backoff (5s, 10s, 20s... capped at 5min) so a persistent
    /// crash loop can't peg the CPU — the backoff resets once a run has
    /// stayed up for a minute, so it's only sustained failures that escalate.
    pub async fn ensure_running(&mut self, monitor_interface: &str) -> Result<()> {
        if self.restart_requested.swap(false, Ordering::SeqCst) && self.child.is_some() {
            tracing::info!("attack settings changed, restarting angryoxide");
            self.stop().await;
            self.restart_count = 0; // a settings change isn't a crash loop
            return self.spawn(monitor_interface).await;
        }

        if let Some(child) = &mut self.child {
            match child.try_wait() {
                Ok(None) => return Ok(()), // still running
                Ok(Some(status)) => {
                    tracing::warn!("angryoxide exited: {status}");
                    self.note_exit();
                }
                Err(e) => {
                    tracing::warn!("angryoxide wait error: {e}");
                    self.note_exit();
                }
            }
        }

        if let Some(started) = self.started_at {
            let backoff =
                Duration::from_secs(5 * 2u64.pow(self.restart_count.min(6))).min(Duration::from_secs(300));
            if started.elapsed() < backoff {
                return Ok(());
            }
        }

        self.spawn(monitor_interface).await
    }

    fn note_exit(&mut self) {
        // A run that stayed up over a minute wasn't a crash loop — don't let
        // it inflate backoff for whatever comes next.
        if self.started_at.map(|s| s.elapsed() > Duration::from_secs(60)) == Some(true) {
            self.restart_count = 0;
        }
        self.child = None;
    }

    async fn spawn(&mut self, monitor_interface: &str) -> Result<()> {
        let capture_dir = PathBuf::from(&self.config.bettercap.handshakes);
        let _ = tokio::fs::create_dir_all(&capture_dir).await;
        let output_base = capture_dir.join("oxigotchi");

        // Read live settings fresh on every spawn (not just at construction)
        // so a change from the web dashboard actually takes effect on the
        // restart ensure_running() just triggered.
        let settings = self
            .settings
            .read()
            .expect("attack settings lock poisoned")
            .clone();

        let mut cmd = Command::new("angryoxide");
        cmd.args([
            "--headless",
            "--notar",
            "-i",
            monitor_interface,
            "-o",
            output_base.to_str().unwrap_or("/etc/pwnagotchi/handshakes/oxigotchi"),
            "-r",
            &settings.rate.clamp(1, 3).to_string(),
            "--whitelist",
            self.whitelist_file.to_str().unwrap_or(""),
        ]);
        if !settings.deauth {
            cmd.arg("--disable-deauth");
        }
        if !settings.pmkid {
            cmd.arg("--disable-pmkid");
        }
        if !settings.anon {
            cmd.arg("--disable-anon");
        }
        if !settings.csa {
            cmd.arg("--disable-csa");
        }
        if !settings.disassoc {
            cmd.arg("--disable-disassoc");
        }
        if !settings.roguem2 {
            cmd.arg("--disable-roguem2");
        }
        cmd.stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true);

        let child = cmd.spawn().context("failed to spawn angryoxide")?;
        tracing::info!("angryoxide started (pid {:?})", child.id());

        self.child = Some(child);
        self.started_at = Some(Instant::now());
        self.restart_count = self.restart_count.saturating_add(1);
        Ok(())
    }

    /// Stop AngryOxide (daemon shutdown or mode change).
    pub async fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill().await;
        }
    }

    pub fn is_running(&mut self) -> bool {
        match &mut self.child {
            Some(child) => matches!(child.try_wait(), Ok(None)),
            None => false,
        }
    }
}
