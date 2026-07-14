use std::sync::Arc;

use anyhow::Result;

use crate::config::Config;
use crate::display::Display;
use crate::epoch::EpochLoop;

mod attacks;
mod bluetooth;
mod capture;
mod config;
mod display;
mod epoch;
mod migration;
mod personality;
mod pisugar;
mod plugins;
mod recovery;
mod web;
mod wifi;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_target(false)
        .init();

    tracing::info!("pwnagotchi-zero {} starting", env!("CARGO_PKG_VERSION"));

    let config_path = std::env::var("PWNAGOTCHI_CONFIG")
        .unwrap_or_else(|_| "/etc/pwnagotchi/config.toml".to_string());
    let config = Arc::new(Config::load(&config_path).await?);
    tracing::info!("config loaded from {}", config_path);

    // --- Signal handling ---------------------------------------------------
    let shutdown = Arc::new(tokio::sync::Notify::new());
    let sig = shutdown.clone();
    tokio::spawn(async move {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{signal, SignalKind};
            let mut sigterm = signal(SignalKind::terminate()).unwrap();
            let mut sigint = signal(SignalKind::interrupt()).unwrap();
            tokio::select! {
                _ = sigterm.recv() => tracing::info!("SIGTERM"),
                _ = sigint.recv() => tracing::info!("SIGINT"),
            }
        }
        #[cfg(not(unix))]
        {
            let _ = tokio::signal::ctrl_c().await;
        }
        sig.notify_waiters();
    });

    // --- Display (boot face) -----------------------------------------------
    let mut display = Display::new(&config)?;
    if let Err(e) = display.init() {
        tracing::warn!("display init (headless): {e}");
    }
    let _ = display.update(true);

    // --- Subsystems --------------------------------------------------------
    let wifi = wifi::WifiManager::new(&config).await?;
    let attacks = attacks::AttackEngine::new(&config).await?;
    let captures = capture::CaptureManager::new(&config).await?;
    let personality = personality::Personality::new(&config).await?;
    let bluetooth = bluetooth::BluetoothManager::new(&config).await?;
    let pisugar = pisugar::PiSugar::new(&config).await?;
    let recovery = recovery::RecoveryManager::new(&config).await?;
    let mut plugins = plugins::PluginManager::new(&config).await?;
    plugins.load_plugins().await.ok();
    let web = web::WebServer::new(&config).await?;

    // --- Web server (background) -------------------------------------------
    // Shares the same live snapshot the epoch loop publishes into.
    if config.ui.web.enabled {
        let wc = config.clone();
        let status = web.status_handle();
        tokio::spawn(async move {
            if let Err(e) = web::WebServer::serve(wc, status).await {
                tracing::error!("web: {e}");
            }
        });
    }

    // --- Epoch loop --------------------------------------------------------
    let mut epoch_loop = EpochLoop::new(
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
    )
    .await?;

    tokio::select! {
        res = epoch_loop.run() => res,
        _ = shutdown.notified() => {
            tracing::info!("shutdown");
            epoch_loop.shutdown().await?;
            Ok(())
        }
    }
}
