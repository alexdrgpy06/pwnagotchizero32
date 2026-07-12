//! Web dashboard server

use anyhow::Result;
use axum::{
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse, Json},
    routing::get,
    Router,
};
use std::sync::Arc;
use tokio::sync::broadcast;

use crate::config::Config;

pub struct WebServer {
    config: Arc<Config>,
    tx: broadcast::Sender<WebEvent>,
}

#[derive(Debug, Clone)]
pub enum WebEvent {
    StatusUpdate(serde_json::Value),
    HandshakeCaptured(serde_json::Value),
    FaceChange(String),
    EpochTick(u64),
}

impl WebServer {
    pub async fn new(config: &Arc<Config>) -> Result<Self> {
        let (tx, _) = broadcast::channel(100);
        Ok(Self {
            config: config.clone(),
            tx,
        })
    }

    /// Start the web server in the foreground. Call from a spawned task.
    pub async fn start_with(config: Arc<Config>) -> Result<()> {
        let router = Router::new()
            .route("/", get(Self::dashboard))
            .route("/api/status", get(Self::api_status))
            .route("/api/shutdown", axum::routing::post(Self::api_shutdown))
            .route("/api/reboot", axum::routing::post(Self::api_reboot))
            .with_state(config.clone());

        let addr = format!("{}:{}", config.ui.web.address, config.ui.web.port);
        let listener = tokio::net::TcpListener::bind(&addr).await?;
        axum::serve(listener, router).await?;
        Ok(())
    }

    async fn dashboard() -> impl IntoResponse {
        Html(include_str!("../../templates/dashboard.html"))
    }

    async fn api_status(State(config): State<Arc<Config>>) -> Json<serde_json::Value> {
        Json(serde_json::json!({
            "name": config.main.name,
            "epoch": 0,
            "mood": "happy",
            "handshakes": 0,
            "battery": 100,
            "bluetooth": false,
            "uptime": 0,
        }))
    }

    async fn api_shutdown() -> impl IntoResponse {
        tokio::spawn(async {
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            let _ = std::process::Command::new("systemctl")
                .args(["poweroff"])
                .output();
        });
        (StatusCode::OK, "Shutting down")
    }

    async fn api_reboot() -> impl IntoResponse {
        tokio::spawn(async {
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            let _ = std::process::Command::new("systemctl")
                .args(["reboot"])
                .output();
        });
        (StatusCode::OK, "Rebooting")
    }
}
