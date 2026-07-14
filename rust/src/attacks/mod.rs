//! Attack engine — spawns and supervises AngryOxide as the WiFi attack and
//! capture tool, replacing the old per-AP aireplay-ng calls.
//!
//! AngryOxide owns the monitor interface once started: it does its own
//! channel hopping, scanning, targeting, and multi-vector attacking
//! (deauth, PMKID, anon-reassoc, CSA, disassoc, rogue M2) in one long-lived
//! process, and writes ready-to-use .hc22000/.pcapng files as it captures.
//! We just keep it alive and let `CaptureManager` pick up its output.

use anyhow::{Context, Result};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::process::{Child, Command};

use crate::config::Config;

pub struct AttackEngine {
    config: Arc<Config>,
    child: Option<Child>,
    whitelist_file: PathBuf,
    started_at: Option<Instant>,
    restart_count: u32,
}

impl AttackEngine {
    pub async fn new(config: &Arc<Config>) -> Result<Self> {
        let whitelist_file = PathBuf::from(&config.bettercap.handshakes).join("angryoxide-whitelist.txt");
        Self::write_whitelist(&whitelist_file, &config.main.whitelist).await?;

        Ok(Self {
            config: config.clone(),
            child: None,
            whitelist_file,
            started_at: None,
            restart_count: 0,
        })
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

        // Our config's attack_rate_limit predates AngryOxide and defaults
        // conservatively (1); reuse it directly as AO's 1-3 aggressiveness
        // knob, so the existing "safe for the shared BCM43436 UART" default
        // still means the least-aggressive setting.
        let rate = self.config.oxigotchi.attack_rate_limit.clamp(1, 3);

        let mut cmd = Command::new("angryoxide");
        cmd.args([
            "--headless",
            "--notar",
            "-i",
            monitor_interface,
            "-o",
            output_base.to_str().unwrap_or("/etc/pwnagotchi/handshakes/oxigotchi"),
            "-r",
            &rate.to_string(),
            "--whitelist",
            self.whitelist_file.to_str().unwrap_or(""),
        ])
        .stdin(Stdio::null())
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
