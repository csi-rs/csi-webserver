use serde::{Deserialize, Serialize};

// ─── Device config (cached state) ─────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DeviceConfig {
    pub wifi_mode: Option<String>,
    pub channel: Option<u32>,
    pub sta_ssid: Option<String>,
    pub traffic_hz: Option<u32>,
    pub collection_mode: Option<String>,
}

// ─── HTTP request bodies ───────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct WifiConfig {
    pub mode: String,
    pub sta_ssid: Option<String>,
    pub sta_password: Option<String>,
    pub channel: Option<u32>,
}

impl WifiConfig {
    pub fn to_cli_command(&self) -> String {
        let mut cmd = format!("set-wifi --mode={}", self.mode);
        if let Some(ssid) = &self.sta_ssid {
            cmd.push_str(&format!(" --sta-ssid={}", ssid.replace(' ', "_")));
        }
        if let Some(pass) = &self.sta_password {
            cmd.push_str(&format!(" --sta-password={}", pass.replace(' ', "_")));
        }
        if let Some(ch) = self.channel {
            cmd.push_str(&format!(" --set-channel={}", ch));
        }
        cmd
    }
}

#[derive(Debug, Deserialize)]
pub struct TrafficConfig {
    pub frequency_hz: u32,
}

impl TrafficConfig {
    pub fn to_cli_command(&self) -> String {
        format!("set-traffic --frequency-hz={}", self.frequency_hz)
    }
}

/// CSI feature flags — non-C6 and C6-specific options are all optional.
/// Only flags set to `true` are included in the generated command.
#[derive(Debug, Deserialize)]
pub struct CsiConfig {
    // Non-C6
    pub disable_lltf: Option<bool>,
    pub disable_htltf: Option<bool>,
    pub disable_stbc_htltf: Option<bool>,
    pub disable_ltf_merge: Option<bool>,
    // C6-specific
    pub disable_csi: Option<bool>,
    pub disable_csi_legacy: Option<bool>,
    pub disable_csi_ht20: Option<bool>,
    pub disable_csi_ht40: Option<bool>,
    pub disable_csi_su: Option<bool>,
    pub disable_csi_mu: Option<bool>,
    pub disable_csi_dcm: Option<bool>,
    pub disable_csi_beamformed: Option<bool>,
    pub csi_he_stbc: Option<u8>,
    pub val_scale_cfg: Option<u8>,
}

impl CsiConfig {
    pub fn to_cli_command(&self) -> String {
        let mut cmd = "set-csi".to_string();
        if self.disable_lltf.unwrap_or(false) {
            cmd.push_str(" --disable-lltf");
        }
        if self.disable_htltf.unwrap_or(false) {
            cmd.push_str(" --disable-htltf");
        }
        if self.disable_stbc_htltf.unwrap_or(false) {
            cmd.push_str(" --disable-stbc-htltf");
        }
        if self.disable_ltf_merge.unwrap_or(false) {
            cmd.push_str(" --disable-ltf-merge");
        }
        if self.disable_csi.unwrap_or(false) {
            cmd.push_str(" --disable-csi");
        }
        if self.disable_csi_legacy.unwrap_or(false) {
            cmd.push_str(" --disable-csi-legacy");
        }
        if self.disable_csi_ht20.unwrap_or(false) {
            cmd.push_str(" --disable-csi-ht20");
        }
        if self.disable_csi_ht40.unwrap_or(false) {
            cmd.push_str(" --disable-csi-ht40");
        }
        if self.disable_csi_su.unwrap_or(false) {
            cmd.push_str(" --disable-csi-su");
        }
        if self.disable_csi_mu.unwrap_or(false) {
            cmd.push_str(" --disable-csi-mu");
        }
        if self.disable_csi_dcm.unwrap_or(false) {
            cmd.push_str(" --disable-csi-dcm");
        }
        if self.disable_csi_beamformed.unwrap_or(false) {
            cmd.push_str(" --disable-csi-beamformed");
        }
        if let Some(stbc) = self.csi_he_stbc {
            cmd.push_str(&format!(" --csi-he-stbc={stbc}"));
        }
        if let Some(scale) = self.val_scale_cfg {
            cmd.push_str(&format!(" --val-scale-cfg={scale}"));
        }
        cmd
    }
}

#[derive(Debug, Deserialize)]
pub struct CollectionModeConfig {
    /// "collector" or "listener"
    pub mode: String,
}

impl CollectionModeConfig {
    pub fn to_cli_command(&self) -> String {
        format!("set-collection-mode --mode={}", self.mode)
    }
}

#[derive(Debug, Deserialize)]
pub struct StartConfig {
    /// Collection duration in seconds; omit for indefinite collection.
    pub duration: Option<u32>,
}

impl StartConfig {
    pub fn to_cli_command(&self) -> String {
        match self.duration {
            Some(d) => format!("start --duration={d}"),
            None => "start".to_string(),
        }
    }
}

// ─── API response ──────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct ApiResponse {
    pub success: bool,
    pub message: String,
}

// ─── CSI packet (parsed from array-list format) ───────────────────────────

/// Parsed representation of one `array-list` line from esp-csi-cli-rs.
///
/// Wire format (one line per packet):
/// `[seq,rssi,rate,noise_floor,ch,ts,sig_len,rx_state,sec_ch,sgi,ant,ampdu,
///   sig_mode,mcs,bw,smooth,not_sound,aggr,stbc,fec,sig_len2,data_len,[csi...]]`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CsiPacket {
    pub sequence_number: u16,
    pub rssi: i32,
    pub rate: u32,
    pub noise_floor: i32,
    pub channel: u32,
    pub timestamp: u32,
    pub sig_len: u32,
    pub rx_state: u32,
    pub secondary_channel: u32,
    pub sgi: u32,
    pub antenna: u32,
    pub ampdu_cnt: u32,
    pub sig_mode: u32,
    pub mcs: u32,
    pub bandwidth: u32,
    pub smoothing: u32,
    pub not_sounding: u32,
    pub aggregation: u32,
    pub stbc: u32,
    pub fec_coding: u32,
    pub csi_data_len: u16,
    pub csi_data: Vec<i8>,
}

fn get_u32(scalars: &[&str], idx: usize) -> Option<u32> {
    scalars.get(idx)?.trim().parse().ok()
}

fn get_i32(scalars: &[&str], idx: usize) -> Option<i32> {
    scalars.get(idx)?.trim().parse().ok()
}

fn get_u16(scalars: &[&str], idx: usize) -> Option<u16> {
    scalars.get(idx)?.trim().parse().ok()
}

impl CsiPacket {
    /// Attempt to parse a single `array-list` output line into a `CsiPacket`.
    /// Returns `None` if the line is not a valid CSI packet.
    pub fn parse_array_list(line: &str) -> Option<Self> {
        let line = line.trim();
        if !line.starts_with('[') || !line.ends_with(']') {
            return None;
        }

        // Strip outermost brackets
        let content = &line[1..line.len() - 1];

        // Locate the nested inner CSI data array [...]
        let inner_start = content.rfind('[')?;
        let inner_end = content.rfind(']')?;
        if inner_start >= inner_end {
            return None;
        }

        // Parse inner CSI i8 samples
        let csi_data: Vec<i8> = content[inner_start + 1..inner_end]
            .split(',')
            .filter_map(|s| s.trim().parse::<i8>().ok())
            .collect();

        // Scalar fields come before the inner array; trim trailing comma/space
        let scalars_str = content[..inner_start].trim_end_matches(|c: char| c == ',' || c.is_whitespace());
        let scalars: Vec<&str> = scalars_str.split(',').collect();

        // Require at least 22 scalar positions (indices 0-21)
        if scalars.len() < 22 {
            return None;
        }

        Some(CsiPacket {
            sequence_number:  get_u16(&scalars, 0)?,
            rssi:             get_i32(&scalars, 1)?,
            rate:             get_u32(&scalars, 2)?,
            noise_floor:      get_i32(&scalars, 3)?,
            channel:          get_u32(&scalars, 4)?,
            timestamp:        get_u32(&scalars, 5)?,
            sig_len:          get_u32(&scalars, 6)?,
            rx_state:         get_u32(&scalars, 7)?,
            secondary_channel: get_u32(&scalars, 8)?,
            sgi:              get_u32(&scalars, 9)?,
            antenna:          get_u32(&scalars, 10)?,
            ampdu_cnt:        get_u32(&scalars, 11)?,
            sig_mode:         get_u32(&scalars, 12)?,
            mcs:              get_u32(&scalars, 13)?,
            bandwidth:        get_u32(&scalars, 14)?,
            smoothing:        get_u32(&scalars, 15)?,
            not_sounding:     get_u32(&scalars, 16)?,
            aggregation:      get_u32(&scalars, 17)?,
            stbc:             get_u32(&scalars, 18)?,
            fec_coding:       get_u32(&scalars, 19)?,
            // scalars[20] = repeated sig_len — intentionally skipped
            csi_data_len:     get_u16(&scalars, 21)?,
            csi_data,
        })
    }
}
