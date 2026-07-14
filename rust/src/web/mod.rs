//! Web dashboard server (axum + htmx/Alpine front-end).
//!
//! Serves a live view of the daemon. All runtime data comes from a
//! [`SharedStatus`] snapshot written by the epoch loop, so the handlers stay
//! decoupled from the individual subsystems.

mod state;

pub use state::{
    new_shared, new_shared_framebuffer, read_system_metrics, SharedFramebuffer, SharedStatus,
    StatusSnapshot,
};

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
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::attacks::{AttackSettings, SharedAttackSettings};
use crate::config::Config;
use crate::display::{DISPLAY_HEIGHT, DISPLAY_WIDTH};

/// Shared router state: config for paths/credentials + the live snapshot.
#[derive(Clone)]
struct AppState {
    config: Arc<Config>,
    status: SharedStatus,
    framebuffer: SharedFramebuffer,
    attack_settings: SharedAttackSettings,
    attack_restart: Arc<AtomicBool>,
}

pub struct WebServer {
    #[allow(dead_code)]
    config: Arc<Config>,
    status: SharedStatus,
    framebuffer: SharedFramebuffer,
}

/// Bytes-per-row of the packed 1bpp framebuffer, matching
/// `display::buffer::FrameBuffer`'s own layout exactly.
const FRAMEBUFFER_STRIDE: usize = (DISPLAY_WIDTH as usize + 7) / 8;
const FRAMEBUFFER_LEN: usize = FRAMEBUFFER_STRIDE * DISPLAY_HEIGHT as usize;

impl WebServer {
    pub async fn new(config: &Arc<Config>) -> Result<Self> {
        Ok(Self {
            config: config.clone(),
            status: new_shared(config.main.name.clone()),
            framebuffer: new_shared_framebuffer(FRAMEBUFFER_LEN),
        })
    }

    /// Handle the epoch loop can use to publish fresh snapshots.
    pub fn status_handle(&self) -> SharedStatus {
        self.status.clone()
    }

    /// Handle the epoch loop can use to publish the live e-ink framebuffer,
    /// so the dashboard can mirror exactly what's on the physical panel.
    pub fn framebuffer_handle(&self) -> SharedFramebuffer {
        self.framebuffer.clone()
    }

    /// Run the HTTP/WebSocket server. Spawn this on its own task.
    pub async fn serve(
        config: Arc<Config>,
        status: SharedStatus,
        framebuffer: SharedFramebuffer,
        attack_settings: SharedAttackSettings,
        attack_restart: Arc<AtomicBool>,
    ) -> Result<()> {
        let state = AppState {
            config: config.clone(),
            status,
            framebuffer,
            attack_settings,
            attack_restart,
        };

        let router = Router::new()
            .route("/", get(dashboard))
            .route("/api/status", get(api_status))
            .route("/api/framebuffer", get(api_framebuffer))
            .route(
                "/api/attacks",
                get(api_attacks_get).post(api_attacks_post),
            )
            .route("/api/handshakes", get(api_handshakes))
            .route(
                "/api/handshakes/download/{file}",
                get(api_handshake_download),
            )
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

/// Raw packed 1bpp framebuffer (MSB-first, 0 bit = black), the exact same
/// bytes the SSD1680 driver was last given — the dashboard mirrors this on a
/// canvas instead of guessing what the physical panel shows.
async fn api_framebuffer(State(state): State<AppState>) -> impl IntoResponse {
    let bytes = state
        .framebuffer
        .read()
        .expect("framebuffer lock poisoned")
        .clone();
    (
        [
            (header::CONTENT_TYPE, "application/octet-stream"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        bytes,
    )
}

async fn api_attacks_get(State(state): State<AppState>) -> Json<AttackSettings> {
    let settings = state
        .attack_settings
        .read()
        .expect("attack settings lock poisoned")
        .clone();
    Json(settings)
}

/// Update attack toggles/rate and ask AttackEngine to restart AngryOxide with
/// them on its next ensure_running() check (every epoch), instead of only
/// taking effect after it happens to crash on its own.
async fn api_attacks_post(
    State(state): State<AppState>,
    Json(new_settings): Json<AttackSettings>,
) -> impl IntoResponse {
    let mut settings = new_settings;
    settings.rate = settings.rate.clamp(1, 3);
    *state
        .attack_settings
        .write()
        .expect("attack settings lock poisoned") = settings.clone();
    state.attack_restart.store(true, Ordering::SeqCst);
    Json(settings)
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
                "client": "-",
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
