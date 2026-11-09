/*
 * C236T Native Controller Library
 *
 * Local LAN control uses Tuya protocol 3.5 (6699/AES-GCM) via `protocol`.
 * The legacy cloud modules (`auth`, `mqtt`) are retained for reference.
 */

pub mod auth;
pub mod config;
pub mod discovery;
pub mod dp;
pub mod mqtt;
pub mod protocol;

pub use auth::InkbirdApiClient;
pub use dp::{
    apply_dp_map, dp_info, dp_label, format_dp, parse_dp_response, C236tState, DpCommand, DpId,
    DpInfo, DpKind, DpValue, HeatCoolingMode, OutputState, SocketState, TimeMode, WorkMode,
    DP_TABLE,
};
pub use config::{canonical_path, find_config_file, load_file_config, parse_config_str, save_file_config, FileConfig};
pub use discovery::{broadcast_targets, discover, local_ipv4, DiscoveredDevice};
pub use mqtt::{C236tMqtt, MqttConfig};
pub use protocol::{dps_from_json, is_timeout, pack_frame, C236tClient, Frame};
