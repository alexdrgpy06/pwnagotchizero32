//! Migration utilities for importing legacy pwnagotchi config

use anyhow::Result;
use std::path::Path;
use std::sync::Arc;

use crate::config::Config;

pub struct MigrationTool {
    config: Arc<Config>,
}

impl MigrationTool {
    pub fn new(config: Arc<Config>) -> Self {
        Self { config }
    }
    
    pub async fn migrate_from_pwnagotchi(&self, old_config: &Path) -> Result<()> {
        // Read old YAML config
        let content = tokio::fs::read_to_string(old_config).await?;
        let old: serde_yaml::Value = serde_yaml::from_str(&content)?;
        
        // Convert to TOML
        let mut new_config = Config::default();
        
        // Map old fields to new
        if let Some(name) = old.get("main").and_then(|m| m.get("name")).and_then(|v| v.as_str()) {
            new_config.main.name = name.to_string();
        }
        
        if let Some(iface) = old.get("main").and_then(|m| m.get("iface")).and_then(|v| v.as_str()) {
            new_config.main.iface = iface.to_string();
        }
        
        if let Some(whitelist) = old.get("main").and_then(|m| m.get("whitelist")).and_then(|v| v.as_sequence()) {
            new_config.main.whitelist = whitelist.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect();
        }
        
        // Migrate plugins
        if let Some(plugins) = old.get("main").and_then(|m| m.get("plugins")).and_then(|v| v.as_mapping()) {
            for (key, value) in plugins {
                if let Some(key_str) = key.as_str() {
                    if let Some(enabled) = value.get("enabled").and_then(|v| v.as_bool()) {
                        new_config.main.plugins.insert(key_str.to_string(), crate::config::PluginConfig {
                            enabled,
                            options: std::collections::HashMap::new(),
                        });
                    }
                }
            }
        }
        
        // Write new config
        let toml = toml::to_string_pretty(&new_config)?;
        let new_path = "/etc/pwnagotchi/config.toml";
        tokio::fs::write(new_path, toml).await?;
        
        Ok(())
    }
    
    pub async fn migrate_handshakes(&self, old_dir: &Path, new_dir: &Path) -> Result<()> {
        // Copy and rename handshake files if needed
        let mut entries = tokio::fs::read_dir(old_dir).await?;
        
        while let Some(entry) = entries.next_entry().await? {
            if entry.path().extension().map_or(false, |ext| ext == "pcapng") {
                let new_path = new_dir.join(entry.file_name());
                tokio::fs::copy(entry.path(), new_path).await?;
            }
        }
        
        Ok(())
    }
}