//! Epoch loop — main state machine

use anyhow::Result;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::web::{read_system_metrics, SharedFramebuffer, SharedStatus};

use crate::attacks::AttackEngine;
use crate::bluetooth::BluetoothManager;
use crate::capture::CaptureManager;
use crate::config::Config;
use crate::display::Display;
use crate::personality::Personality;
use crate::pisugar::PiSugar;
use crate::plugins::PluginManager;
use crate::recovery::RecoveryManager;
use crate::web::WebServer;
use crate::wifi::WifiManager;

pub struct EpochLoop {
    config: Arc<Config>,
    display: Display,
    wifi: WifiManager,
    attacks: AttackEngine,
    captures: CaptureManager,
    personality: Personality,
    bluetooth: BluetoothManager,
    pisugar: PiSugar,
    recovery: RecoveryManager,
    plugins: PluginManager,
    web: WebServer,
    status: SharedStatus,
    framebuffer: SharedFramebuffer,
    start: Instant,
    epoch: u64,
    running: bool,
}

impl EpochLoop {
    #[allow(clippy::too_many_arguments)]
    pub async fn new(
        config: Arc<Config>,
        display: Display,
        wifi: WifiManager,
        attacks: AttackEngine,
        captures: CaptureManager,
        personality: Personality,
        bluetooth: BluetoothManager,
        pisugar: PiSugar,
        recovery: RecoveryManager,
        plugins: PluginManager,
        web: WebServer,
    ) -> Result<Self> {
        let status = web.status_handle();
        let framebuffer = web.framebuffer_handle();
        Ok(Self {
            config,
            display,
            wifi,
            attacks,
            captures,
            personality,
            bluetooth,
            pisugar,
            recovery,
            plugins,
            web,
            status,
            framebuffer,
            start: Instant::now(),
            epoch: 0,
            running: true,
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        // Init display
        if let Err(e) = self.display.init() {
            tracing::warn!("display init (headless): {e}");
        }

        // Start monitor mode, then hand the interface to AngryOxide — it owns
        // scanning/channel-hopping/attacking from here.
        if let Err(e) = self.wifi.start_monitor_mode().await {
            tracing::warn!("monitor mode: {e}");
        }
        if let Err(e) = self.attacks.ensure_running(self.wifi.monitor_interface()).await {
            tracing::warn!("angryoxide start: {e}");
        }

        // Bluetooth PAN (best-effort)
        let phone = &self.config.oxigotchi.phone_mac;
        if !phone.is_empty() {
            let _ = self.bluetooth.connect_pan(phone).await;
        }

        // Notify plugins we're loaded
        let status = self.build_status();
        self.plugins.on_epoch(self.epoch, &status).ok();

        while self.running {
            self.run_epoch().await?;
            // The epoch's real yield point: without an .await here, this loop
            // never returns control to the tokio scheduler, so the SIGTERM
            // handler and web server tasks never get polled — the daemon
            // becomes deaf to shutdown and systemd has to SIGKILL it after
            // the full stop timeout. This also paces attacks/display updates
            // to the configured epoch duration instead of running flat out.
            let secs = self.config.oxigotchi.epoch_duration.max(1) as u64;
            tokio::time::sleep(Duration::from_secs(secs)).await;
        }

        Ok(())
    }

    async fn run_epoch(&mut self) -> Result<()> {
        self.epoch += 1;

        // Phase 1: AngryOxide owns scanning/channel-hopping/attacking once
        // started; this just restarts it (with backoff) if it has crashed.
        if let Err(e) = self.attacks.ensure_running(self.wifi.monitor_interface()).await {
            tracing::warn!("angryoxide health check: {e}");
        }

        // Phase 2: pick up any new capture files AngryOxide has written, and
        // give the bull his mood/XP boost for each one.
        match self.captures.scan_new_captures().await {
            Ok(new_files) => {
                for _ in &new_files {
                    self.personality.update_on_handshake();
                }
                if !new_files.is_empty() {
                    tracing::info!("captured {} new handshake(s)", new_files.len());
                }
            }
            Err(e) => tracing::warn!("capture scan: {e}"),
        }

        // Phase 3: update display
        self.update_display()?;

        // Phase 4: maintenance
        self.maintenance().await?;

        // Publish live snapshot for the web dashboard
        self.publish_status();

        // Notify plugins
        let status = self.build_status();
        self.plugins.on_epoch(self.epoch, &status)?;

        Ok(())
    }

    /// Push the current runtime state into the shared snapshot the web
    /// dashboard reads from.
    fn publish_status(&self) {
        let stats = self.personality.get_stats();
        let (cpu_temp, ram_used, ram_total) = read_system_metrics();

        if let Ok(mut snap) = self.status.write() {
            snap.epoch = self.epoch;
            snap.mood = format!("{:?}", stats.mood).to_lowercase();
            snap.face = self.personality.get_face(stats.mood);
            snap.handshakes = stats.handshakes;
            snap.aps_found = self.wifi.get_aps().len();
            snap.channel = self.wifi.current_channel();
            snap.blind_epochs = self.personality.blind_epochs();
            snap.level = stats.level;
            snap.xp = stats.xp;
            snap.battery = self.pisugar.battery_percent();
            snap.charging = self.pisugar.is_charging();
            snap.bluetooth = self.bluetooth.is_connected();
            snap.bt_device = self.bluetooth.current_device().map(str::to_string);
            snap.cpu_temp = cpu_temp;
            snap.ram_used = ram_used;
            snap.ram_total = ram_total;
            snap.uptime = self.start.elapsed().as_secs();
        }
    }

    fn update_display(&mut self) -> Result<()> {
        // Start from a blank frame; drawing only sets black pixels, so without
        // this the previous face and status text would remain underneath.
        self.display.clear()?;

        // The exact pwnagotchi layout — coordinates from jayofelony's own
        // waveshare2in13_V4.py hw driver for this exact panel, real Unicode
        // kaomoji face (TTF-rendered, matching pwnagotchi's own PIL-based
        // UI) instead of an ASCII approximation. Replaces the bare face +
        // one raw debug line this used to draw
        // ("Epoch:1 HS:0 BT:off Bat:100%"), which showed none of the
        // information an actual pwnagotchi display shows.
        let stats = self.personality.get_stats();
        let face = self.personality.get_face(stats.mood);
        let secs = self.start.elapsed().as_secs();
        let uptime = format!("{:02}:{:02}:{:02}", secs / 3600, (secs % 3600) / 60, secs % 60);
        let (cpu_temp, ram_used, ram_total) = read_system_metrics();
        self.display.draw_pwnagotchi_frame(
            self.wifi.current_channel(),
            self.wifi.get_aps().len(),
            self.bluetooth.is_connected(),
            &uptime,
            &self.config.main.name,
            self.personality.get_phrase(),
            &face,
            stats.handshakes,
            stats.level,
            "AUTO",
            cpu_temp,
            ram_used,
            ram_total,
        )?;

        // Partial refresh most epochs; force a full (de-ghosting) refresh on
        // the first epoch and every `display_full_refresh_interval` after, or
        // always-full when partial refresh is disabled in config.
        //
        // self.epoch is incremented to 1 before this runs, so "first epoch"
        // is epoch == 1, not epoch == 0 — `epoch % interval == 0` alone never
        // matches on the very first call (1 % 10 == 1), so the real content
        // (mood + status line) never appeared until 10 epochs (~5 minutes)
        // of uninterrupted running had passed; every partial refresh before
        // that redraws a whole new face + status line through a LUT meant
        // for small incremental changes, which doesn't reliably apply
        // visually on this hardware. The panel was stuck showing whatever
        // the last full refresh drew — the boot face from Display::init().
        let cfg = &self.config.oxigotchi;
        let interval = cfg.display_full_refresh_interval;
        let due_full = self.epoch == 1 || (interval != 0 && self.epoch % interval == 0);
        let partial = cfg.display_partial_refresh && !due_full;
        self.display.update(partial)?;

        // Publish for the web dashboard's live e-ink mirror — same bytes
        // just sent to the panel, so the browser shows exactly what's there.
        if let Ok(mut fb) = self.framebuffer.write() {
            fb.copy_from_slice(self.display.framebuffer_bytes());
        }

        Ok(())
    }

    async fn maintenance(&mut self) -> Result<()> {
        // PiSugar
        let _ = self.pisugar.update().await;
        // Low battery shutdown — only when a PiSugar is actually present, so a
        // missing/unreadable UPS can never shut the device down on a phantom 0%.
        if self.pisugar.is_present()
            && self.pisugar.battery_percent() < 10
            && !self.pisugar.is_charging()
        {
            tracing::warn!("low battery, shutting down");
            let _ = self.shutdown().await;
        }
        // WiFi recovery check
        let _ = self.recovery.check().await;
        Ok(())
    }

    fn build_status(&self) -> crate::plugins::EpochStatus {
        crate::plugins::EpochStatus {
            epoch: self.epoch,
            channel: self.wifi.current_channel(),
            aps_found: self.wifi.get_aps().len(),
            handshakes: self.personality.get_stats().handshakes as usize,
            battery: self.pisugar.battery_percent(),
        }
    }

    pub async fn shutdown(&mut self) -> Result<()> {
        self.running = false;
        self.attacks.stop().await;
        self.display.show_shutdown()?;
        // EpdDriver::sleep is sync now
        self.display.sleep()?;
        Ok(())
    }
}
