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
            self.run_epoch()?;
        }

        Ok(())
    }

    fn run_epoch(&mut self) -> Result<()> {
        self.epoch += 1;

        // Phase 1-2: scan + attack
        let aps = self.wifi.get_aps().to_vec();
        for ap in &aps {
            if !self.wifi.is_whitelisted(&ap.ssid, &ap.bssid) {
                let _ = self.attacks.deauth(ap, None);
            }
        }

        // Phase 3: check captures — real daemon handles this via inotify / polling
        // Phase 4: update display
        self.update_display()?;

        // Phase 5: maintenance
        self.maintenance()?;

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
        let stats = self.personality.get_stats();
        let mood_str = format!("{:?}", stats.mood).to_lowercase();
        self.display.draw_face(&mood_str);

        let status = format!(
            "Epoch:{} HS:{} BT:{} Bat:{}%",
            self.epoch,
            stats.handshakes,
            if self.bluetooth.is_connected() { "↑" } else { "↓" },
            self.pisugar.battery_percent(),
        );
        self.display.draw_status_line(100, &status)?;

        // Partial refresh most epochs, full every 10
        let partial = self.epoch % 10 != 0;
        self.display.update(partial)?;

        Ok(())
    }

    fn maintenance(&mut self) -> Result<()> {
        // PiSugar
        let _ = futures::executor::block_on(self.pisugar.update());
        // Low battery shutdown
        if self.pisugar.battery_percent() < 10 && !self.pisugar.is_charging() {
            tracing::warn!("low battery, shutting down");
            let _ = self.shutdown();
        }
        // WiFi recovery check
        let _ = futures::executor::block_on(self.recovery.check());
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
