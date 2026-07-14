//! Capture manager for pcapng handshake files

use anyhow::Result;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::fs;
use tokio::process::Command;

use crate::config::Config;
use crate::wifi::AccessPoint;

/// Capture file entry
#[derive(Debug, Clone)]
pub struct CaptureFile {
    pub path: PathBuf,
    pub ap: AccessPoint,
    pub client_mac: String,
    pub timestamp: u64,
    pub handshake_type: HandshakeType,
    pub uploaded: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum HandshakeType {
    PMKID,
    FullHandshake,
    HalfHandshake,
}

/// Capture manager
pub struct CaptureManager {
    config: Arc<Config>,
    capture_dir: PathBuf,
    upload_queue: VecDeque<CaptureFile>,
    max_captures: usize,
}

impl CaptureManager {
    pub async fn new(config: &Arc<Config>) -> Result<Self> {
        let capture_dir = PathBuf::from(&config.bettercap.handshakes);
        fs::create_dir_all(&capture_dir).await?;

        Ok(Self {
            config: config.clone(),
            capture_dir,
            upload_queue: VecDeque::new(),
            max_captures: 1000,
        })
    }

    /// Start capture for an AP (handled by hcxdumptool externally)
    pub async fn start_capture(&self, ap: &AccessPoint) -> Result<PathBuf> {
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        let filename = format!("{}_{}.pcapng", ap.bssid.replace(':', ""), timestamp);
        let path = self.capture_dir.join(filename);

        // hcxdumptool is run externally, we just track the file
        Ok(path)
    }

    /// Register a captured handshake
    pub async fn register_handshake(
        &mut self,
        path: PathBuf,
        ap: AccessPoint,
        client_mac: String,
        handshake_type: HandshakeType,
    ) -> Result<()> {
        let capture = CaptureFile {
            path: path.clone(),
            ap: ap.clone(),
            client_mac,
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
            handshake_type,
            uploaded: false,
        };

        self.upload_queue.push_back(capture);

        // Cleanup old captures if over limit
        self.cleanup_old().await?;

        Ok(())
    }

    /// Scan the capture directory for new AngryOxide-produced .hc22000 files
    /// (AO writes these directly, ready for hashcat — no conversion needed)
    /// and register any we haven't already seen. AngryOxide's exact filename
    /// scheme isn't something we depend on here — we only need to detect
    /// that a new capture appeared, not parse its AP identity precisely.
    /// Returns the paths of newly registered captures, so the caller can
    /// drive mood/XP updates per new handshake.
    pub async fn scan_new_captures(&mut self) -> Result<Vec<PathBuf>> {
        let mut new_files = Vec::new();
        let mut dir = fs::read_dir(&self.capture_dir).await?;
        while let Some(entry) = dir.next_entry().await? {
            let path = entry.path();
            let is_hc22000 = path.extension().map_or(false, |e| e == "hc22000");
            if !is_hc22000 || self.upload_queue.iter().any(|c| c.path == path) {
                continue;
            }
            new_files.push(path);
        }

        for path in &new_files {
            let bssid = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string();
            let ap = AccessPoint {
                bssid,
                ssid: String::new(),
                channel: 0,
                rssi: 0,
                encryption: String::new(),
                vendor: String::new(),
            };
            self.register_handshake(path.clone(), ap, String::new(), HandshakeType::FullHandshake)
                .await?;
        }

        Ok(new_files)
    }

    /// Convert pcapng to hccapx for hashcat
    pub async fn to_hccapx(&self, pcapng_path: &Path) -> Result<PathBuf> {
        let hccapx_path = pcapng_path.with_extension("hccapx");

        let output = Command::new("hcxpcapngtool")
            .args([
                "-o",
                hccapx_path.to_str().unwrap(),
                pcapng_path.to_str().unwrap(),
            ])
            .output()
            .await?;

        if !output.status.success() {
            anyhow::bail!(
                "hcxpcapngtool failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        Ok(hccapx_path)
    }

    /// Get pending uploads
    pub fn pending_uploads(&self) -> Vec<&CaptureFile> {
        self.upload_queue.iter().filter(|c| !c.uploaded).collect()
    }

    /// Mark as uploaded
    pub fn mark_uploaded(&mut self, path: &Path) {
        for capture in &mut self.upload_queue {
            if capture.path == path {
                capture.uploaded = true;
                break;
            }
        }
    }

    async fn cleanup_old(&mut self) -> Result<()> {
        let mut entries = Vec::new();
        let mut dir = fs::read_dir(&self.capture_dir).await?;
        while let Some(entry) = dir.next_entry().await? {
            if entry
                .path()
                .extension()
                .map_or(false, |ext| ext == "pcapng")
            {
                entries.push(entry);
            }
        }
        // Sort by modification time (pre-fetch metadata)
        let mut with_mtime: Vec<_> = Vec::new();
        for entry in &entries {
            let mtime = entry
                .metadata()
                .await?
                .modified()
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            with_mtime.push((entry.path().to_owned(), mtime));
        }
        with_mtime.sort_by_key(|(_, mtime)| *mtime);

        while entries.len() > self.max_captures {
            if let Some(oldest_path) = with_mtime.first().map(|(p, _)| p.clone()) {
                fs::remove_file(&oldest_path).await?;
                entries.retain(|e| e.path() != oldest_path);
                with_mtime.remove(0);
            }
        }

        Ok(())
    }

    pub async fn get_stats(&self) -> CaptureStats {
        let total = self.upload_queue.len();
        let uploaded = self.upload_queue.iter().filter(|c| c.uploaded).count();
        let pending = total - uploaded;

        CaptureStats {
            total_captures: total,
            uploaded,
            pending,
            disk_usage: self.calculate_disk_usage().await.unwrap_or(0),
        }
    }

    async fn calculate_disk_usage(&self) -> Result<u64> {
        let mut total = 0u64;
        let mut dir = fs::read_dir(&self.capture_dir).await?;
        while let Some(entry) = dir.next_entry().await? {
            total += entry.metadata().await?.len();
        }
        Ok(total)
    }
}

#[derive(Debug, Clone)]
pub struct CaptureStats {
    pub total_captures: usize,
    pub uploaded: usize,
    pub pending: usize,
    pub disk_usage: u64,
}
