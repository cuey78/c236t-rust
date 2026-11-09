/*
 * Data Point (DP) definitions for C236T
 * 
 * All DP IDs and value mappings reverse-engineered from the INKBIRD APK.
 * Source: com/inkbird/base/db/bean/c236t/C236tConstant.java
 *         com/inkbird/inkbirdapp/device/c236t/utils/C236tManager.java
 *         com/inkbird/base/common/ConstantDevicePID.java
 */

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// C236T Product IDs
pub const PRODUCT_ID_C236T: &str = "ae6rmrnkqz7rjh66";
pub const PRODUCT_ID_C236T_INKTEM: &str = "fhxzbdcssszg1pc5";
pub const PRODUCT_ID_C236T_OTA: &str = "giwst7t3cnb7q7xz";
pub const PRODUCT_ID_C236T_OTA_INKTEM: &str = "6mbrvmttev3t0dei";

/// All DP IDs for C236T (as strings, since they're sent as JSON keys)
pub mod DpId {
    pub const QUERY_ALL: &str = "101";   // Request all DPs from device
    pub const GUIDE_SW: &str = "102";     // Skip guide flag
    pub const WORK_MODE: &str = "103";    // Work mode (0-3)
    pub const TIME_MODE: &str = "104";    // Time mode (0-3)
    pub const TEMP_TIME_MODE: &str = "105"; // Temp-time mode (0-3)
    pub const UNIT_F: &str = "106";       // Temperature unit (true=°F, false=°C)
    pub const TEMP_HEAT_START: &str = "107"; // Heat start temp (-40 to 100°C)
    pub const TEMP_HEAT_STOP: &str = "108";  // Heat stop temp (-40 to 100°C)
    pub const ALARM_HIGH_TEMP: &str = "109"; // High alarm temp (HA)
    pub const ALARM_LOW_TEMP: &str = "110";  // Low alarm temp (LA)
    pub const COOLING_DELAY: &str = "111";   // Cooling delay seconds
    pub const TEMP_CALIBRATION: &str = "112"; // Temp calibration offset (tenths)
    pub const ALARM_HIGH_SW: &str = "113";   // High alarm enabled
    pub const ALARM_LOW_SW: &str = "114";    // Low alarm enabled
    pub const DISPLAY_SW: &str = "115";      // Display/backlight on
    pub const CHILD_LOCK: &str = "116";      // Child lock
    pub const TEMP_CTRL_SW: &str = "117";     // Temperature control enable
    pub const MANUAL_POWER: &str = "118";     // Manual socket power (on/off)
    pub const PROGRAM_CTRL_SW: &str = "119";  // Program/schedule control enable
    pub const BUZZER_SW: &str = "120";       // Buzzer enabled
    pub const CTRL_SET: &str = "121";        // Time schedule periods P01-P08 (hex string)
    pub const CURRENT_TEMP: &str = "129";    // Current measured temp (read-only)
    pub const OUTPUT_STATE: &str = "130";    // Output state (read-only): HEAT_OFF/ON, COOL_OFF/ON
    pub const ALARM_HISTORY: &str = "131";   // Alarm history
    pub const HISTORY_RECORD: &str = "132";  // History record (hex string)
    pub const TIME_WORK_INFO: &str = "133";  // Time work info (hex string)
    pub const TEMP_COOL_START: &str = "134"; // Cool start temp
    pub const TEMP_COOL_STOP: &str = "135";  // Cool stop temp
    pub const TEMP_TIME_HEAT_START: &str = "136"; // Temp-time heat start
    pub const TEMP_TIME_HEAT_STOP: &str = "137"; // Temp-time heat stop
    pub const TEMP_TIME_COOL_START: &str = "138"; // Temp-time cool start
    pub const TEMP_TIME_COOL_STOP: &str = "139"; // Temp-time cool stop
    pub const ALARM: &str = "145";           // Alarm state (read-only, hex string)
    pub const COOL_DELAY_SW: &str = "146";   // Cool delay switch
    pub const CLEAR_DEV_DATA: &str = "147";  // Clear device data
    pub const MUTE_ALARM: &str = "148";      // Mute device alarm
    pub const SKIP_GUIDE: &str = "102";      // Skip guide flag (same as GUIDE_SW)
}

/// Work mode values (DP 103)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum WorkMode {
    #[default]
    TempMode = 0,      // Simple temperature control
    TimeMode = 1,      // Time-based control
    TempTimeMode = 2,  // Combined temp + time
    ManualMode = 3,    // Manual override
}

impl From<i32> for WorkMode {
    fn from(v: i32) -> Self {
        match v {
            0 => WorkMode::TempMode,
            1 => WorkMode::TimeMode,
            2 => WorkMode::TempTimeMode,
            3 => WorkMode::ManualMode,
            _ => WorkMode::TempMode,
        }
    }
}

/// Time mode values (DP 104, 105)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum TimeMode {
    #[default]
    CountdownCycle = 0,
    CountdownOn = 1,
    CountdownOff = 2,
    CountdownOnAndOff = 3,
}

impl From<i32> for TimeMode {
    fn from(v: i32) -> Self {
        match v {
            0 => TimeMode::CountdownCycle,
            1 => TimeMode::CountdownOn,
            2 => TimeMode::CountdownOff,
            3 => TimeMode::CountdownOnAndOff,
            _ => TimeMode::CountdownCycle,
        }
    }
}

/// Heat/Cooling mode (DP 117)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum HeatCoolingMode {
    #[default]
    Heating = 0,
    Cooling = 1,
}

/// Output state (DP 130, read-only)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum OutputState {
    #[default]
    HeatOff,
    HeatOn,
    CoolOff,
    CoolOn,
}

impl From<&str> for OutputState {
    fn from(s: &str) -> Self {
        match s {
            "HEAT_ON" => OutputState::HeatOn,
            "HEAT_OFF" => OutputState::HeatOff,
            "COOL_ON" => OutputState::CoolOn,
            "COOL_OFF" => OutputState::CoolOff,
            _ => OutputState::HeatOff,
        }
    }
}

/// Alarm type (DP 145, parsed from hex string)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum AlarmState {
    #[default]
    No,
    Er,       // Error
    Ah,       // High alarm
    Al,       // Low alarm
}

impl AlarmState {
    fn from_hex_type(hex_type: i32) -> Self {
        match hex_type {
            0 => AlarmState::No,
            1 => AlarmState::Er,
            2 => AlarmState::Ah,
            3 => AlarmState::Al,
            _ => AlarmState::No,
        }
    }
}

/// Socket/power state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SocketState {
    #[default]
    PowerOff,
    PowerOn,
}

/// DP value types that can be sent/received
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DpValue {
    Bool(bool),
    Int(i32),
    String(String),
    Float(f64),
}

/// DP command: send a single DP to the device
#[derive(Debug, Clone, Serialize)]
pub struct DpCommand {
    #[serde(flatten)]
    pub value: DpValue,
}

/// Convenience: create a DP command map
pub fn dp_map(dp_id: &str, value: DpValue) -> HashMap<String, DpValue> {
    let mut map = HashMap::new();
    map.insert(dp_id.to_string(), value);
    map
}

/// Full device state parsed from DP updates
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct C236tState {
    pub dev_id: String,
    pub product_id: String,
    pub unit_f: bool,
    pub work_mode: WorkMode,
    pub time_mode: TimeMode,
    pub temp_time_mode: TimeMode,
    pub temp_heat_start: i32,
    pub temp_heat_stop: i32,
    pub temp_cool_start: i32,
    pub temp_cool_stop: i32,
    pub temp_time_heat_start: i32,
    pub temp_time_heat_stop: i32,
    pub temp_time_cool_start: i32,
    pub temp_time_cool_stop: i32,
    pub alarm_high_temp: i32,
    pub alarm_low_temp: i32,
    pub cooling_delay: i32,
    pub temp_calibration: i32,
    pub current_temp: i32,
    pub alarm_high_open: bool,
    pub alarm_low_open: bool,
    pub display_open: bool,
    pub child_lock: bool,
    pub heat_cooling_mode: HeatCoolingMode,
    pub buz_open: bool,
    pub cool_delay_open: bool,
    pub socket_state: SocketState,
    pub manual_power_state: SocketState,
    pub temp_ctrl_sw: bool,
    pub temp_state: OutputState,
    pub alarm_state: AlarmState,
    pub skip_guide: i32,
    pub time_work_switch: bool,
    pub total_on_time: i64,
    pub total_off_time: i64,
    pub current_run_time: i64,
    pub power_on_work_time: i64,
    pub total_heat_time: i64,
    pub total_cool_time: i64,
    pub temp_c: i32,
    pub temp_f: i32,
}

/// Parse a DP JSON update string into a HashMap
/// The JSON looks like: {"129": 23, "103": 0, "106": false, "130": "HEAT_ON", ...}
pub fn parse_dp_response(json_str: &str) -> Result<HashMap<String, serde_json::Value>, serde_json::Error> {
    serde_json::from_str(json_str)
}

/// Helper: extract a bool from DP map
pub fn get_bool(map: &HashMap<String, serde_json::Value>, key: &str) -> Option<bool> {
    map.get(key).and_then(|v| v.as_bool())
}

/// Helper: extract an i32 from DP map
pub fn get_i32(map: &HashMap<String, serde_json::Value>, key: &str) -> Option<i32> {
    map.get(key).and_then(|v| v.as_i64()).map(|v| v as i32)
}

/// Helper: extract a string from DP map
pub fn get_str(map: &HashMap<String, serde_json::Value>, key: &str) -> Option<String> {
    map.get(key).and_then(|v| v.as_str()).map(|s| s.to_string())
}

/// Apply a parsed DP map to update device state
pub fn apply_dp_map(state: &mut C236tState, map: &HashMap<String, serde_json::Value>) {
    if let Some(v) = get_bool(map, DpId::UNIT_F) { state.unit_f = v; }
    if let Some(v) = get_i32(map, DpId::WORK_MODE) { state.work_mode = WorkMode::from(v); }
    if let Some(v) = get_i32(map, DpId::TIME_MODE) { state.time_mode = TimeMode::from(v); }
    if let Some(v) = get_i32(map, DpId::TEMP_TIME_MODE) { state.temp_time_mode = TimeMode::from(v); }
    if let Some(v) = get_i32(map, DpId::TEMP_HEAT_START) { state.temp_heat_start = v; }
    if let Some(v) = get_i32(map, DpId::TEMP_HEAT_STOP) { state.temp_heat_stop = v; }
    if let Some(v) = get_i32(map, DpId::TEMP_COOL_START) { state.temp_cool_start = v; }
    if let Some(v) = get_i32(map, DpId::TEMP_COOL_STOP) { state.temp_cool_stop = v; }
    if let Some(v) = get_i32(map, DpId::TEMP_TIME_HEAT_START) { state.temp_time_heat_start = v; }
    if let Some(v) = get_i32(map, DpId::TEMP_TIME_HEAT_STOP) { state.temp_time_heat_stop = v; }
    if let Some(v) = get_i32(map, DpId::TEMP_TIME_COOL_START) { state.temp_time_cool_start = v; }
    if let Some(v) = get_i32(map, DpId::TEMP_TIME_COOL_STOP) { state.temp_time_cool_stop = v; }
    if let Some(v) = get_i32(map, DpId::ALARM_HIGH_TEMP) { state.alarm_high_temp = v; }
    if let Some(v) = get_i32(map, DpId::ALARM_LOW_TEMP) { state.alarm_low_temp = v; }
    if let Some(v) = get_i32(map, DpId::COOLING_DELAY) { state.cooling_delay = v; }
    if let Some(v) = get_i32(map, DpId::TEMP_CALIBRATION) { state.temp_calibration = v; }
    if let Some(v) = get_i32(map, DpId::CURRENT_TEMP) { state.current_temp = v; }
    if let Some(v) = get_bool(map, DpId::ALARM_HIGH_SW) { state.alarm_high_open = v; }
    if let Some(v) = get_bool(map, DpId::ALARM_LOW_SW) { state.alarm_low_open = v; }
    if let Some(v) = get_bool(map, DpId::DISPLAY_SW) { state.display_open = v; }
    if let Some(v) = get_bool(map, DpId::CHILD_LOCK) { state.child_lock = v; }
    if let Some(v) = get_bool(map, DpId::BUZZER_SW) { state.buz_open = v; }
    if let Some(v) = get_bool(map, DpId::COOL_DELAY_SW) { state.cool_delay_open = v; }
    if let Some(v) = get_bool(map, DpId::MANUAL_POWER) {
        state.manual_power_state = if v { SocketState::PowerOn } else { SocketState::PowerOff };
    }
    if let Some(v) = get_bool(map, DpId::TEMP_CTRL_SW) { state.temp_ctrl_sw = v; }
    if let Some(v) = get_str(map, DpId::OUTPUT_STATE) {
        state.temp_state = OutputState::from(v.as_str());
        state.heat_cooling_mode = if v.starts_with("COOL") {
            HeatCoolingMode::Cooling
        } else {
            HeatCoolingMode::Heating
        };
    }
    if let Some(v) = get_i32(map, DpId::SKIP_GUIDE) { state.skip_guide = v; }
    if let Some(v) = get_bool(map, DpId::PROGRAM_CTRL_SW) {
        state.time_work_switch = v;
    }

    // Parse alarm type from DP 145 (hex string, alarm type at bytes 14-16)
    if let Some(alarm_hex) = get_str(map, DpId::ALARM) {
        if alarm_hex.len() >= 16 {
            let hex_type = i32::from_str_radix(&alarm_hex[14..16], 16).unwrap_or(0);
            state.alarm_state = AlarmState::from_hex_type(hex_type);
        }
    }

    // Parse DP 132 (history record, hex string) - runtime stats
    if let Some(history) = get_str(map, DpId::HISTORY_RECORD) {
        if history.len() >= 58 {
            state.power_on_work_time = i64::from_str_radix(&history[0..8], 16).unwrap_or(0);
            state.total_on_time = i64::from_str_radix(&history[16..24], 16).unwrap_or(0);
            state.total_off_time = i64::from_str_radix(&history[24..32], 16).unwrap_or(0);
            state.current_run_time = i64::from_str_radix(&history[34..42], 16).unwrap_or(0);
            state.total_heat_time = i64::from_str_radix(&history[42..50], 16).unwrap_or(0);
            state.total_cool_time = i64::from_str_radix(&history[50..58], 16).unwrap_or(0);
        }
    }
}

/// How a DP value should be displayed / parsed on the CLI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DpKind {
    Bool,
    Int,
    /// Signed integer in hundredths of a degree.
    Temp,
    Text,
    /// Write-only trigger (e.g. clear data / mute alarm).
    Momentary,
}

pub struct DpInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub kind: DpKind,
    pub description: &'static str,
}

/// Every DP the C236T firmware exposes, as mapped from
/// `com.inkbird.base.db.bean.c236t.C236tConstant`.
pub const DP_TABLE: &[DpInfo] = &[
    DpInfo { id: "101", name: "all", kind: DpKind::Momentary, description: "Query / report all data points" },
    DpInfo { id: "102", name: "guide_sw", kind: DpKind::Int, description: "Guide / setup wizard step" },
    DpInfo { id: "103", name: "workMode", kind: DpKind::Int, description: "Work mode: 0 temp, 1 time, 2 temp+time, 3 manual" },
    DpInfo { id: "104", name: "timeMode", kind: DpKind::Int, description: "Time mode: 0 cycle, 1 countdown on, 2 countdown off, 3 on+off" },
    DpInfo { id: "105", name: "tempTimeMode", kind: DpKind::Int, description: "Temp+time mode: 0 cycle, 1 on, 2 off, 3 on+off" },
    DpInfo { id: "106", name: "unit", kind: DpKind::Bool, description: "Temperature unit: false = Celsius, true = Fahrenheit" },
    DpInfo { id: "107", name: "temp_heatStart", kind: DpKind::Temp, description: "Heating start temperature" },
    DpInfo { id: "108", name: "temp_heatStop", kind: DpKind::Temp, description: "Heating stop temperature" },
    DpInfo { id: "109", name: "ah_alarm", kind: DpKind::Temp, description: "High temperature alarm threshold" },
    DpInfo { id: "110", name: "al_alarm", kind: DpKind::Temp, description: "Low temperature alarm threshold" },
    DpInfo { id: "111", name: "cool_cd", kind: DpKind::Int, description: "Cooling delay (seconds)" },
    DpInfo { id: "112", name: "ca", kind: DpKind::Int, description: "Temperature calibration offset" },
    DpInfo { id: "113", name: "ah_sw", kind: DpKind::Bool, description: "High temperature alarm enabled" },
    DpInfo { id: "114", name: "al_sw", kind: DpKind::Bool, description: "Low temperature alarm enabled" },
    DpInfo { id: "115", name: "bg_light_sw", kind: DpKind::Bool, description: "Backlight / display enabled" },
    DpInfo { id: "116", name: "kid_lock", kind: DpKind::Bool, description: "Child lock" },
    DpInfo { id: "117", name: "temp_ctrl_sw", kind: DpKind::Bool, description: "Temperature control enabled" },
    DpInfo { id: "118", name: "manual_sw", kind: DpKind::Bool, description: "Manual output power" },
    DpInfo { id: "119", name: "P_ctrl_sw", kind: DpKind::Bool, description: "Program / schedule control enabled" },
    DpInfo { id: "120", name: "buz_sw", kind: DpKind::Bool, description: "Buzzer enabled" },
    DpInfo { id: "121", name: "P_ctrl_set", kind: DpKind::Text, description: "Program schedule (8 x 10 hex chars)" },
    DpInfo { id: "129", name: "current_temp", kind: DpKind::Temp, description: "Measured temperature (read-only)" },
    DpInfo { id: "130", name: "output_mode", kind: DpKind::Text, description: "Output state: HEAT_ON/HEAT_OFF/COOL_ON/COOL_OFF (read-only)" },
    DpInfo { id: "131", name: "alarm_history", kind: DpKind::Text, description: "Alarm history (read-only)" },
    DpInfo { id: "132", name: "history_record", kind: DpKind::Text, description: "Runtime statistics (read-only)" },
    DpInfo { id: "133", name: "timeWorkInfo", kind: DpKind::Text, description: "Time-work statistics (read-only)" },
    DpInfo { id: "134", name: "temp_coolStart", kind: DpKind::Temp, description: "Cooling start temperature" },
    DpInfo { id: "135", name: "temp_coolStop", kind: DpKind::Temp, description: "Cooling stop temperature" },
    DpInfo { id: "136", name: "tempTime_heatStart", kind: DpKind::Temp, description: "Temp+time heating start temperature" },
    DpInfo { id: "137", name: "tempTime_heatStop", kind: DpKind::Temp, description: "Temp+time heating stop temperature" },
    DpInfo { id: "138", name: "tempTime_coolStart", kind: DpKind::Temp, description: "Temp+time cooling start temperature" },
    DpInfo { id: "139", name: "tempTime_coolStop", kind: DpKind::Temp, description: "Temp+time cooling stop temperature" },
    DpInfo { id: "145", name: "alarm", kind: DpKind::Text, description: "Current alarm (read-only)" },
    DpInfo { id: "146", name: "cool_delay_sw", kind: DpKind::Bool, description: "Cooling delay enabled" },
    DpInfo { id: "147", name: "clear_dev_data", kind: DpKind::Momentary, description: "Clear device data (write-only trigger)" },
    DpInfo { id: "148", name: "mute_now_dev_alarm", kind: DpKind::Momentary, description: "Mute the current alarm (write-only trigger)" },
    DpInfo { id: "149", name: "reserved_149", kind: DpKind::Text, description: "Reserved / firmware-specific DP" },
    DpInfo { id: "150", name: "ota_update", kind: DpKind::Bool, description: "Firmware OTA update available/trigger" },
];

/// Look up a DP by numeric ID or by APK field name (case-insensitive).
pub fn dp_info(key: &str) -> Option<&'static DpInfo> {
    DP_TABLE
        .iter()
        .find(|d| d.id == key || d.name.eq_ignore_ascii_case(key))
}

/// Human-readable label for a DP ID.
pub fn dp_label(id: &str) -> String {
    match dp_info(id) {
        Some(info) => format!("{} ({})", info.name, info.id),
        None => format!("unknown DP {id}"),
    }
}

/// Format a DP value for display, taking the DP kind into account.
pub fn format_dp(info: Option<&DpInfo>, value: &serde_json::Value) -> String {
    use serde_json::Value;
    let kind = info.map(|d| d.kind).unwrap_or(DpKind::Int);
    match (kind, value) {
        (DpKind::Bool, Value::Bool(b)) => {
            if *b { "on".to_string() } else { "off".to_string() }
        }
        (DpKind::Temp, Value::Number(n)) => {
            let raw = n.as_f64().unwrap_or(0.0);
            format!("{:.2} (raw {})", raw / 100.0, n)
        }
        (_, Value::String(s)) => s.clone(),
        (_, v) => v.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_dp() {
        let json = r#"{"129":25,"103":0,"106":false,"130":"HEAT_ON","113":true,"114":false}"#;
        let map = parse_dp_response(json).unwrap();
        assert_eq!(map.get("129").unwrap().as_i64(), Some(25));
        assert_eq!(map.get("103").unwrap().as_i64(), Some(0));
        assert_eq!(map.get("106").unwrap().as_bool(), Some(false));
        assert_eq!(map.get("130").unwrap().as_str(), Some("HEAT_ON"));
    }

    #[test]
    fn test_apply_dp_map() {
        let json = r#"{"129":23,"103":1,"106":true,"107":25,"108":30,"130":"COOL_ON","117":true}"#;
        let map = parse_dp_response(json).unwrap();
        let mut state = C236tState::default();
        apply_dp_map(&mut state, &map);
        assert_eq!(state.current_temp, 23);
        assert_eq!(state.work_mode, WorkMode::TimeMode);
        assert!(state.unit_f);
        assert_eq!(state.temp_heat_start, 25);
        assert_eq!(state.temp_heat_stop, 30);
        assert_eq!(state.temp_state, OutputState::CoolOn);
        assert_eq!(state.heat_cooling_mode, HeatCoolingMode::Cooling);
        assert!(state.temp_ctrl_sw);
    }

    #[test]
    fn test_output_state_parsing() {
        assert_eq!(OutputState::from("HEAT_ON"), OutputState::HeatOn);
        assert_eq!(OutputState::from("HEAT_OFF"), OutputState::HeatOff);
        assert_eq!(OutputState::from("COOL_ON"), OutputState::CoolOn);
        assert_eq!(OutputState::from("COOL_OFF"), OutputState::CoolOff);
    }

    #[test]
    fn test_alarm_state_from_hex() {
        // DP 145 hex: alarm type at bytes 14-16 (0-indexed chars 14..16)
        // "0000000000000002" -> alarm type 2 = Ah (high alarm)
        let hex = "0000000000000002";
        let hex_type = i32::from_str_radix(&hex[14..16], 16).unwrap();
        assert_eq!(hex_type, 2);
        assert_eq!(AlarmState::from_hex_type(hex_type), AlarmState::Ah);
    }
}
