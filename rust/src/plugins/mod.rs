//! Plugin manager for Lua and native plugins

use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use crate::config::Config;

/// Plugin manager supporting Lua and native plugins
pub struct PluginManager {
    config: Arc<Config>,
    native_plugins: HashMap<String, Box<dyn NativePlugin>>,
}

#[allow(unused_variables)]
pub trait NativePlugin: Send + Sync {
    fn name(&self) -> &str;
    fn version(&self) -> &str {
        "1.0.0"
    }
    fn on_loaded(&mut self, ctx: &PluginContext) -> Result<()> {
        Ok(())
    }
    fn on_unload(&mut self) -> Result<()> {
        Ok(())
    }
    fn on_epoch(&mut self, ctx: &PluginContext, epoch: u64, status: &EpochStatus) -> Result<()> {
        Ok(())
    }
    fn on_handshake(
        &mut self,
        ctx: &PluginContext,
        path: &Path,
        ap: &AccessPoint,
        client: &Client,
    ) -> Result<()> {
        Ok(())
    }
    fn on_internet_available(&mut self, ctx: &PluginContext) -> Result<()> {
        Ok(())
    }
    fn on_ui_update(&mut self, ui: &mut UiContext) -> Result<()> {
        Ok(())
    }
}

pub struct PluginContext {
    pub config: Arc<Config>,
}

pub struct EpochStatus {
    pub epoch: u64,
    pub channel: u8,
    pub aps_found: usize,
    pub handshakes: usize,
    pub battery: u8,
}

pub struct AccessPoint {
    pub bssid: String,
    pub ssid: String,
    pub channel: u8,
    pub rssi: i8,
}

pub struct Client {
    pub mac: String,
    pub ap_bssid: String,
}

pub struct UiContext {}

impl PluginManager {
    pub async fn new(config: &Arc<Config>) -> Result<Self> {
        Ok(Self {
            config: config.clone(),
            native_plugins: HashMap::new(),
        })
    }

    pub async fn load_plugins(&mut self) -> Result<()> {
        let plugin_dir = Path::new(&self.config.main.custom_plugins);
        if plugin_dir.exists() {
            for entry in std::fs::read_dir(plugin_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.extension().map_or(false, |ext| ext == "lua") {
                    tracing::info!("found lua plugin: {:?}", path);
                    // Lua plugins need mlua integration; deferred to a dedicated PR.
                }
            }
        }
        Ok(())
    }

    pub fn register_native_plugin(&mut self, plugin: Box<dyn NativePlugin>) {
        let name = plugin.name().to_string();
        self.native_plugins.insert(name, plugin);
    }

    pub fn on_epoch(&mut self, epoch: u64, status: &EpochStatus) -> Result<()> {
        let ctx = PluginContext {
            config: self.config.clone(),
        };
        for (_, plugin) in &mut self.native_plugins {
            plugin.on_epoch(&ctx, epoch, status)?;
        }
        Ok(())
    }

    pub fn on_handshake(&mut self, path: &Path, ap: &AccessPoint, client: &Client) -> Result<()> {
        let ctx = PluginContext {
            config: self.config.clone(),
        };
        for (_, plugin) in &mut self.native_plugins {
            plugin.on_handshake(&ctx, path, ap, client)?;
        }
        Ok(())
    }

    pub fn on_internet_available(&mut self) -> Result<()> {
        let ctx = PluginContext {
            config: self.config.clone(),
        };
        for (_, plugin) in &mut self.native_plugins {
            plugin.on_internet_available(&ctx)?;
        }
        Ok(())
    }

    pub fn on_ui_update(&mut self, ui: &mut UiContext) -> Result<()> {
        for (_, plugin) in &mut self.native_plugins {
            plugin.on_ui_update(ui)?;
        }
        Ok(())
    }
}
