use tokio::fs::OpenOptions;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::{broadcast, mpsc, watch};
use tokio_serial::{SerialPortBuilderExt, SerialPortType};

use crate::models::OutputMode;

const DEFAULT_BAUD_RATE: u32 = 115_200;

/// Known ESP32 USB-UART adapter Vendor IDs.
const ESP_USB_VIDS: &[u16] = &[
    0x10C4, // Silicon Labs CP210x (most common on ESP32 devkits)
    0x1A86, // WCH CH340 / CH341
    0x303A, // Espressif built-in USB (ESP32-S3 / C3 / C6 native USB)
];

/// Detect the first available ESP32 USB serial port.
///
/// Resolution order:
/// 1. `CSI_SERIAL_PORT` environment variable override.
/// 2. First USB port whose name contains `usbserial` / `usbmodem` / `ttyUSB` / `ttyACM`,
///    or whose VID matches a known ESP chip.
/// 3. Any USB port as a last resort.
pub fn detect_esp_port() -> Result<String, String> {
    // Allow the user to pin a specific port without recompiling.
    if let Ok(port) = std::env::var("CSI_SERIAL_PORT") {
        tracing::info!("Using CSI_SERIAL_PORT override: {port}");
        return Ok(port);
    }

    let ports = tokio_serial::available_ports()
        .map_err(|e| format!("Failed to enumerate serial ports: {e}"))?;

    // First pass: match by known VID or recognisable port-name prefix.
    for port in &ports {
        if let SerialPortType::UsbPort(ref info) = port.port_type {
            let name_ok = port.port_name.contains("usbserial")
                || port.port_name.contains("usbmodem")
                || port.port_name.contains("ttyUSB")
                || port.port_name.contains("ttyACM");

            let vid_ok = ESP_USB_VIDS.contains(&info.vid);

            if name_ok || vid_ok {
                let product = info
                    .product
                    .as_deref()
                    .map(|p| format!(", {p}"))
                    .unwrap_or_default();
                tracing::info!(
                    "Auto-detected ESP port: {} (VID:{:04X} PID:{:04X}{product})",
                    port.port_name,
                    info.vid,
                    info.pid,
                );
                return Ok(port.port_name.clone());
            }
        }
    }

    // Second pass: fall back to any USB port.
    for port in &ports {
        if matches!(port.port_type, SerialPortType::UsbPort(_)) {
            tracing::warn!(
                "No known ESP port found — using first USB port: {}",
                port.port_name
            );
            return Ok(port.port_name.clone());
        }
    }

    let names: Vec<&str> = ports.iter().map(|p| p.port_name.as_str()).collect();
    Err(format!(
        "No USB serial ports detected. Available ports: [{}]",
        names.join(", ")
    ))
}

/// Background task: owns the serial port for its lifetime.
///
/// - Reads incoming frames from the serial port and broadcasts the raw bytes
///   to all WebSocket subscribers via `csi_tx`. The frame delimiter adapts to
///   the active log mode: `\0` for COBS, `\n` for all text-based modes.
/// - Watches `cmd_rx` for outgoing CLI command strings and writes them to the
///   port, appending a newline.
/// - Does NOT set a log mode on startup — call `POST /api/config/log-mode` to
///   configure the device before collecting data.
pub async fn run_serial_task(
    port_path: String,
    mut cmd_rx: mpsc::Receiver<String>,
    csi_tx: broadcast::Sender<Vec<u8>>,
    log_mode_rx: watch::Receiver<String>,
    mut output_mode_rx: watch::Receiver<OutputMode>,
    mut session_file_rx: watch::Receiver<Option<String>>,
) {
    let baud = std::env::var("CSI_BAUD_RATE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_BAUD_RATE);

    let stream = match tokio_serial::new(&port_path, baud).open_native_async() {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("Failed to open serial port {port_path}: {e}");
            return;
        }
    };

    tracing::info!("Opened serial port {port_path} @ {baud} baud");


    let (reader, mut writer) = tokio::io::split(stream);
    let mut reader = BufReader::new(reader);
    let mut buf = Vec::new();

    // ── Dump-file state (owned exclusively by this task) ──────────────────
    let mut current_mode = OutputMode::Stream;
    let mut current_session_path: Option<String> = None;
    let mut dump_file: Option<tokio::fs::File> = None;

    loop {
        // ── React to runtime output-mode or session-file changes ──────────
        let mode_changed = output_mode_rx.has_changed().unwrap_or(false);
        let session_changed = session_file_rx.has_changed().unwrap_or(false);

        if mode_changed {
            current_mode = output_mode_rx.borrow_and_update().clone();
        }
        if session_changed {
            match session_file_rx.borrow_and_update().clone() {
                Some(path) => current_session_path = Some(path),
                None => {
                    // Session ended — close the dump file.
                    dump_file = None;
                    current_session_path = None;
                    tracing::info!("Session ended — dump file closed");
                }
            }
        }
        if mode_changed || session_changed {
            match current_mode {
                OutputMode::Dump | OutputMode::Both => {
                    if dump_file.is_none() {
                        if let Some(ref path) = current_session_path {
                            match OpenOptions::new()
                                .write(true)
                                .create(true)
                                .truncate(true)
                                .open(path)
                                .await
                            {
                                Ok(f) => {
                                    tracing::info!("Opened dump file: {path}");
                                    dump_file = Some(f);
                                }
                                Err(e) => {
                                    tracing::error!("Failed to open dump file {path}: {e}");
                                }
                            }
                        }
                    }
                }
                OutputMode::Stream => {
                    // Drop the file handle; the file is flushed on drop.
                    if dump_file.take().is_some() {
                        tracing::info!("Switched to stream mode — dump file closed");
                    }
                }
            }
        }

        // Pick the frame delimiter based on the current log mode.
        // COBS uses null-byte (0x00) framing; all text modes use newline.
        let delimiter = {
            let mode = log_mode_rx.borrow();
            if mode.to_ascii_lowercase().contains("cobs") {
                b'\0'
            } else {
                b'\n'
            }
        };

        tokio::select! {
            // ── Incoming serial data ──────────────────────────────────────
            result = reader.read_until(delimiter, &mut buf) => {
                match result {
                    Ok(0) => {
                        tracing::warn!("Serial port {port_path} closed (EOF)");
                        break;
                    }
                    Ok(_) => {
                        // Strip the trailing delimiter before forwarding.
                        if buf.last() == Some(&delimiter) {
                            buf.pop();
                        }
                        if !buf.is_empty() {
                            // Write to dump file (u32 LE length-prefix + frame bytes).
                            if matches!(current_mode, OutputMode::Dump | OutputMode::Both) {
                                if let Some(ref mut file) = dump_file {
                                    let len = buf.len() as u32;
                                    if let Err(e) = file.write_all(&len.to_le_bytes()).await {
                                        tracing::error!("Dump write error (len): {e}");
                                    } else if let Err(e) = file.write_all(&buf).await {
                                        tracing::error!("Dump write error (data): {e}");
                                    }
                                }
                            }
                            // Broadcast to WebSocket clients.
                            if matches!(current_mode, OutputMode::Stream | OutputMode::Both) {
                                // Ignore send errors — zero subscribers is fine.
                                let _ = csi_tx.send(buf.clone());
                            }
                        }
                        buf.clear();
                    }
                    Err(e) => {
                        tracing::error!("Serial read error on {port_path}: {e}");
                        break;
                    }
                }
            }

            // ── Outgoing command ──────────────────────────────────────────
            cmd = cmd_rx.recv() => {
                match cmd {
                    Some(cmd) => {
                        tracing::debug!("→ ESP32: {cmd}");
                        let line = format!("{cmd}\n");
                        if let Err(e) = writer.write_all(line.as_bytes()).await {
                            tracing::error!("Serial write error: {e}");
                            break;
                        }
                    }
                    None => {
                        tracing::info!("Command channel closed — shutting down serial task");
                        break;
                    }
                }
            }
        }
    }
}
