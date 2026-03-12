use std::sync::Arc;
use tokio::sync::{Mutex, broadcast, mpsc, watch};

use crate::models::{DeviceConfig, OutputMode};

/// Shared application state, cheaply cloned into every route handler via Axum's `State` extractor.
#[derive(Clone)]
pub struct AppState {
    /// USB serial port path used to reach the ESP32 (e.g. `/dev/ttyUSB0`).
    /// Stored so route handlers can open a short-lived second fd for control
    /// operations such as RTS-triggered reset.
    pub port_path: Arc<String>,
    /// Send CLI command strings to the serial background task.
    pub cmd_tx: mpsc::Sender<String>,
    /// Broadcast raw CSI frame bytes to all connected WebSocket clients.
    pub csi_tx: broadcast::Sender<Vec<u8>>,
    /// Notify the serial task of log-mode changes (affects the frame delimiter).
    pub log_mode_tx: Arc<watch::Sender<String>>,
    /// Notify the serial task of output-mode changes (stream / dump / both).
    pub output_mode_tx: Arc<watch::Sender<OutputMode>>,
    /// Signal the serial task of the current session's dump file path.
    /// `Some(path)` → open/reuse that file; `None` → session ended, close file.
    pub session_file_tx: Arc<watch::Sender<Option<String>>>,
    /// Cached view of the current device configuration.
    pub config: Arc<Mutex<DeviceConfig>>,
}
