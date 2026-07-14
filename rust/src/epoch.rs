//! Epoch loop — main state machine

use anyhow::Result;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::web::{read_system_metrics, SharedStatus};

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

        // Start monitor mode
        if let Err(e) = self.wifi.start_monitor_mode().await {
            tracing::warn!("monitor mode: {e}");
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

        // Phase 1-2: scan + attack
        let aps = self.wifi.get_aps().to_vec();
        for ap in &aps {
            if !self.wifi.is_whitelisted(&ap.ssid, &ap.bssid) {
                let _ = self.attacks.deauth(ap, None).await;
            }
        }

        // Phase 3: check captures — real daemon handles this via inotify / polling
        // Phase 4: update display
        self.update_display()?;

        // Phase 5: maintenance
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

        let stats = self.personality.get_stats();
        let mood_str = format!("{:?}", stats.mood).to_lowercase();
        self.display.draw_face(&mood_str);

        let status = format!(
            "Epoch:{} HS:{} BT:{} Bat:{}%",
            self.epoch,
            stats.handshakes,
            if self.bluetooth.is_connected() {
                "on"
            } else {
                "off"
            },
            self.pisugar.battery_percent(),
        );
        self.display.draw_status_line(100, &status)?;

        // Partial refresh most epochs; force a full (de-ghosting) refresh on
        // the first epoch and every `display_full_refresh_interval` after, or
        // always-full when partial refresh is disabled in config.
        let cfg = &self.config.oxigotchi;
        let interval = cfg.display_full_refresh_interval;
        let due_full = interval != 0 && self.epoch % interval == 0;
        let partial = cfg.display_partial_refresh && !due_full;
        self.display.update(partial)?;

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
            let _ = self.shutdown();
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

    pub fn shutdown(&mut self) -> Result<()> {
        self.running = false;
        self.display.show_shutdown()?;
        // EpdDriver::sleep is sync now
        self.display.sleep()?;
        Ok(())
    }
}
