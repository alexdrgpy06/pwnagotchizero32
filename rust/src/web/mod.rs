//! Web dashboard server (axum + htmx/Alpine front-end).
//!
//! Serves a live view of the daemon. All runtime data comes from a
//! [`SharedStatus`] snapshot written by the epoch loop, so the handlers stay
//! decoupled from the individual subsystems.

mod state;

pub use state::{new_shared, read_system_metrics, SharedStatus, StatusSnapshot};

use anyhow::Result;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path as AxumPath, State,
    },
    http::{header, StatusCode},
    response::{Html, IntoResponse, Json},
    routing::{get, post},
    Router,
};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use crate::config::Config;

/// Shared router state: config for paths/credentials + the live snapshot.
#[derive(Clone)]
struct AppState {
    config: Arc<Config>,
    status: SharedStatus,
}

pub struct WebServer {
    #[allow(dead_code)]
    config: Arc<Config>,
    status: SharedStatus,
}

impl WebServer {
    pub async fn new(config: &Arc<Config>) -> Result<Self> {
        Ok(Self {
            config: config.clone(),
            status: new_shared(config.main.name.clone()),
        })
    }

    /// Handle the epoch loop can use to publish fresh snapshots.
    pub fn status_handle(&self) -> SharedStatus {
        self.status.clone()
    }

    /// Run the HTTP/WebSocket server. Spawn this on its own task.
    pub async fn serve(config: Arc<Config>, status: SharedStatus) -> Result<()> {
        let state = AppState { config: config.clone(), status };

        let router = Router::new()
            .route("/", get(dashboard))
            .route("/api/status", get(api_status))
            .route("/api/handshakes", get(api_handshakes))
            .route("/api/handshakes/download/{file}", get(api_handshake_download))
            .route("/api/shutdown", post(api_shutdown))
            .route("/api/reboot", post(api_reboot))
            .route("/ws", get(ws_upgrade))
            .with_state(state);

        let addr = format!("{}:{}", config.ui.web.address, config.ui.web.port);
        let listener = tokio::net::TcpListener::bind(&addr).await?;
        tracing::info!("web dashboard listening on {addr}");
        axum::serve(listener, router).await?;
        Ok(())
    }
}

async fn dashboard() -> impl IntoResponse {
    Html(include_str!("../../templates/dashboard.html"))
}

async fn api_status(State(state): State<AppState>) -> Json<StatusSnapshot> {
    let snap = state.status.read().expect("status lock poisoned").clone();
    Json(snap)
}

/// List handshake capture files currently on disk.
async fn api_handshakes(State(state): State<AppState>) -> Json<Vec<serde_json::Value>> {
    let dir = PathBuf::from(&state.config.bettercap.handshakes);
    let mut out = Vec::new();

    if let Ok(mut rd) = tokio::fs::read_dir(&dir).await {
        while let Ok(Some(entry)) = rd.next_entry().await {
            let path = entry.path();
            let is_capture = path
                .extension()
                .map(|e| e == "pcapng" || e == "pcap")
                .unwrap_or(false);
            if !is_capture {
                continue;
            }
            let file = match path.file_name().and_then(|n| n.to_str()) {
                Some(f) => f.to_string(),
                None => continue,
            };
            // Filenames are "<bssid-no-colons>_<unix-ts>.pcapng".
            let stem = file.trim_end_matches(".pcapng").trim_end_matches(".pcap");
            let (bssid, ts) = stem.split_once('_').unwrap_or((stem, "0"));
            out.push(serde_json::json!({
                "path": file,
                "ap": format_bssid(bssid),
                "client": "—",
                "time": ts,
                "type": "handshake",
            }));
        }
    }

    // Newest first (filenames carry the unix timestamp).
    out.sort_by(|a, b| b["time"].as_str().cmp(&a["time"].as_str()));
    Json(out)
}

/// Stream a single capture file back to the browser (basename only, no traversal).
async fn api_handshake_download(
    State(state): State<AppState>,
    AxumPath(file): AxumPath<String>,
) -> impl IntoResponse {
    let name = std::path::Path::new(&file)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_string();
    if name.is_empty() {
        return (StatusCode::BAD_REQUEST, "invalid file").into_response();
    }

    let path = PathBuf::from(&state.config.bettercap.handshakes).join(&name);
    match tokio::fs::read(&path).await {
        Ok(bytes) => (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, "application/octet-stream".to_string()),
                (
                    header::CONTENT_DISPOSITION,
                    format!("attachment; filename=\"{name}\""),
                ),
            ],
            bytes,
        )
            .into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "not found").into_response(),
    }
}

async fn api_shutdown() -> impl IntoResponse {
    tokio::spawn(async {
        tokio::time::sleep(Duration::from_secs(1)).await;
        let _ = tokio::process::Command::new("systemctl")
            .args(["poweroff"])
            .output()
            .await;
    });
    (StatusCode::OK, "Shutting down")
}

async fn api_reboot() -> impl IntoResponse {
    tokio::spawn(async {
        tokio::time::sleep(Duration::from_secs(1)).await;
        let _ = tokio::process::Command::new("systemctl")
            .args(["reboot"])
            .output()
            .await;
    });
    (StatusCode::OK, "Rebooting")
}

/// Upgrade to a WebSocket that pushes the latest status roughly every 2s.
async fn ws_upgrade(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| ws_loop(socket, state.status.clone()))
}

async fn ws_loop(mut socket: WebSocket, status: SharedStatus) {
    let mut tick = tokio::time::interval(Duration::from_secs(2));
    loop {
        tick.tick().await;
        let payload = {
            let snap = match status.read() {
                Ok(s) => s.clone(),
                Err(_) => break,
            };
            match serde_json::to_value(&snap) {
                Ok(mut v) => {
                    v["type"] = serde_json::Value::String("status".into());
                    v.to_string()
                }
                Err(_) => continue,
            }
        };
        if socket.send(Message::Text(payload.into())).await.is_err() {
            break; // client disconnected
        }
    }
}

/// Turn "AABBCCDDEEFF" back into "AA:BB:CC:DD:EE:FF" for display.
fn format_bssid(raw: &str) -> String {
    if raw.len() == 12 && raw.chars().all(|c| c.is_ascii_hexdigit()) {
        raw.as_bytes()
            .chunks(2)
            .map(|c| std::str::from_utf8(c).unwrap_or("").to_string())
            .collect::<Vec<_>>()
            .join(":")
            .to_uppercase()
    } else {
        raw.to_string()
    }
}
