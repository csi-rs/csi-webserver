mod models;
mod routes;
mod serial;
mod state;

use std::sync::Arc;

use axum::{
    Router,
    routing::{get, post},
};
use tokio::sync::{Mutex, broadcast, mpsc};

use models::DeviceConfig;
use state::AppState;

#[tokio::main]
async fn main() {
    // ── Tracing ───────────────────────────────────────────────────────────
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "csi_webserver=debug".into()),
        )
        .init();

    // ── Serial port detection ─────────────────────────────────────────────
    let port_path = match serial::detect_esp_port() {
        Ok(p) => p,
        Err(e) => {
            tracing::error!("{e}");
            std::process::exit(1);
        }
    };

    // ── Channels ──────────────────────────────────────────────────────────
    // cmd_tx  → serial task (CLI commands)
    // csi_tx  → all WebSocket clients (parsed CSI JSON)
    let (cmd_tx, cmd_rx) = mpsc::channel::<String>(64);
    let (csi_tx, _) = broadcast::channel::<String>(256);

    // ── Shared state ──────────────────────────────────────────────────────
    let state = AppState {
        cmd_tx,
        csi_tx: csi_tx.clone(),
        config: Arc::new(Mutex::new(DeviceConfig::default())),
    };

    // ── Serial background task ────────────────────────────────────────────
    tokio::spawn(serial::run_serial_task(port_path, cmd_rx, csi_tx));

    // ── Router ────────────────────────────────────────────────────────────
    let app = Router::new()
        .route("/",                             get(|| async { "CSI Server Active" }))
        // Config
        .route("/api/config",                   get(routes::config::get_config))
        .route("/api/config/reset",             post(routes::config::reset_config))
        .route("/api/config/wifi",              post(routes::config::set_wifi))
        .route("/api/config/traffic",           post(routes::config::set_traffic))
        .route("/api/config/csi",               post(routes::config::set_csi))
        .route("/api/config/collection-mode",   post(routes::config::set_collection_mode))
        // Control
        .route("/api/control/start",            post(routes::control::start_collection))
        // WebSocket
        .route("/api/ws",                       get(routes::ws::ws_handler))
        .with_state(state);

    // ── Serve ─────────────────────────────────────────────────────────────
    let addr = "0.0.0.0:3000";
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    tracing::info!("CSI server listening on http://{addr}");
    axum::serve(listener, app).await.unwrap();
}