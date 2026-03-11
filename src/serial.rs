use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::{broadcast, mpsc};
use tokio_serial::{SerialPortBuilderExt, SerialPortType};

use crate::models::CsiPacket;

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
                    port.port_name, info.vid, info.pid,
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
/// - Reads incoming lines; those that parse as CSI packets are JSON-serialised
///   and broadcast to all WebSocket subscribers via `csi_tx`.
/// - Watches `cmd_rx` for outgoing CLI command strings and writes them to the
///   port, appending a newline.
/// - Enforces `array-list` log mode immediately after opening the port.
pub async fn run_serial_task(
    port_path: String,
    mut cmd_rx: mpsc::Receiver<String>,
    csi_tx: broadcast::Sender<String>,
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
    let mut lines = BufReader::new(reader).lines();

    // Enforce array-list mode on startup so we always receive parseable output.
    if let Err(e) = writer.write_all(b"set-log-mode --mode=array-list\n").await {
        tracing::error!("Failed to send init command to {port_path}: {e}");
        return;
    }
    tracing::info!("Sent init: set-log-mode --mode=array-list");

    loop {
        tokio::select! {
            // ── Incoming serial data ──────────────────────────────────────
            result = lines.next_line() => {
                match result {
                    Ok(Some(line)) => {
                        match CsiPacket::parse_array_list(&line) {
                            Some(packet) => {
                                match serde_json::to_string(&packet) {
                                    Ok(json) => {
                                        // Ignore errors — zero subscribers is fine.
                                        let _ = csi_tx.send(json);
                                    }
                                    Err(e) => {
                                        tracing::warn!("Failed to serialise CSI packet: {e}");
                                    }
                                }
                            }
                            None => {
                                // Non-CSI device output (boot messages, errors, etc.)
                                tracing::trace!("ESP32: {line}");
                            }
                        }
                    }
                    Ok(None) => {
                        tracing::warn!("Serial port {port_path} closed (EOF)");
                        break;
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
