use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, Mutex};

use crate::models::DeviceConfig;

/// Shared application state, cheaply cloned into every route handler via Axum's `State` extractor.
#[derive(Clone)]
pub struct AppState {
    /// Send CLI command strings to the serial background task.
    pub cmd_tx: mpsc::Sender<String>,
    /// Broadcast parsed CSI JSON strings to all connected WebSocket clients.
    pub csi_tx: broadcast::Sender<String>,
    /// Cached view of the current device configuration.
    pub config: Arc<Mutex<DeviceConfig>>,
}
