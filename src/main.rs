/*
 * C236T Native Controller - local LAN control over Tuya protocol 3.5.
 *
 * Device:  INKBIRD C236T (configured via c236t.toml)
 *
 * Usage:
 *   c236t-ctl status
 *   c236t-ctl get 129
 *   c236t-ctl set 107=25 108=30
 *   c236t-ctl set temp_ctrl_sw=on
 *   c236t-ctl watch
 *   c236t-ctl raw '{"dps":{"101":false}}'
 *   c236t-ctl list
 *   c236t-ctl discover
 *   c236t-ctl setup
 *
 * Configuration (c236t.toml, env vars, or legacy .env):
 *   local_key / THING_LOCAL_KEY   device local key (required)
 *   ip / THING_LOCAL_IP           device LAN address (required)
 *   port / THING_LOCAL_PORT       device LAN port (default 6668)
 *   device_id / THING_DEV_ID      device ID (optional, never prompted)
 *   C236T_CONFIG_FILE             explicit config file path
 */

use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use c236t_ctl::config as file_config;
use c236t_ctl::discovery;
use c236t_ctl::dp::{self, DpKind, DpInfo, DP_TABLE};
use c236t_ctl::protocol::{self, C236tClient};

static SHELL_ENV: OnceLock<HashMap<String, String>> = OnceLock::new();

fn main() {
    // Exit quietly when stdout is closed early (e.g. `... | head`).
    #[cfg(unix)]
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }

    // Snapshot shell env before .env so config() can tell shell vars
    // (highest priority) apart from .env values (fallback).
    let shell_snapshot: HashMap<String, String> = [
        "THING_LOCAL_KEY",
        "THING_LOCAL_IP",
        "THING_LOCAL_PORT",
        "THING_DEV_ID",
        "C236T_CONFIG_FILE",
        "THING_CONFIG_FILE",
    ]
    .iter()
    .filter_map(|k| std::env::var(k).ok().map(|v| (k.to_string(), v)))
    .collect();
    SHELL_ENV.get_or_init(|| shell_snapshot);

    dotenv::dotenv().ok();

    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        usage();
        std::process::exit(2);
    }

    let result = match args[0].as_str() {
        "status" => config().and_then(|cfg| cmd_status(&cfg)),
        "get" => config().and_then(|cfg| cmd_get(&cfg, args.get(1))),
        "set" => config().and_then(|cfg| cmd_set(&cfg, &args[1..])),
        "watch" => config().and_then(|cfg| cmd_watch(&cfg)),
        "raw" => config().and_then(|cfg| cmd_raw(&cfg, args.get(1))),
        "list" => {
            cmd_list();
            Ok(())
        }
        "discover" => cmd_discover(&args[1..]),
        "setup" => cmd_setup(&args[1..]),
        "help" | "-h" | "--help" => {
            usage();
            Ok(())
        }
        other => Err(format!("unknown command `{other}` (try `help`)")),
    };

    if let Err(err) = result {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

struct Config {
    ip: String,
    port: u16,
    local_key: Vec<u8>,
    dev_id: String,
}

fn shell_var(name: &str) -> Option<String> {
    SHELL_ENV
        .get()
        .and_then(|m| m.get(name).cloned())
}

fn config() -> Result<Config, String> {
    // Precedence per field: shell env > c236t.toml > .env > built-in default.
    let file_cfg = file_config::load_file_config()
        .map_err(|e| e)? .unwrap_or_default();

    let resolved_key = shell_var("THING_LOCAL_KEY")
        .or_else(|| file_cfg.local_key.clone())
        .or_else(|| std::env::var("THING_LOCAL_KEY").ok());
    let local_key =
        resolved_key.ok_or_else(|| "no device local key found (run `c236t-ctl setup` or create c236t.toml)".to_string())?;
    if local_key.as_bytes().len() != 16 {
        return Err(format!(
            "device local key must be 16 bytes, got {}",
            local_key.as_bytes().len()
        ));
    }
    let ip = shell_var("THING_LOCAL_IP")
        .or_else(|| file_cfg.ip.clone())
        .or_else(|| std::env::var("THING_LOCAL_IP").ok())
        .ok_or_else(|| "no device IP found (run `c236t-ctl discover` + `setup`, or set `ip` in c236t.toml)".to_string())?;
    let port = shell_var("THING_LOCAL_PORT")
        .and_then(|p| p.parse().ok())
        .or(file_cfg.port)
        .or_else(|| {
            std::env::var("THING_LOCAL_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
        })
        .unwrap_or(6668);
    if port == 0 {
        return Err("invalid port `0` (1..=65535)".to_string());
    }
    let dev_id = shell_var("THING_DEV_ID")
        .or_else(|| file_cfg.device_id.clone())
        .or_else(|| std::env::var("THING_DEV_ID").ok())
        .unwrap_or_else(|| "unknown".to_string());

    Ok(Config {
        ip,
        port,
        local_key: local_key.into_bytes(),
        dev_id,
    })
}

fn connect(cfg: &Config) -> Result<C236tClient, String> {
    C236tClient::connect(&cfg.ip, cfg.port, &cfg.local_key)
        .map_err(|e| format!("could not connect to {}:{} - {e}", cfg.ip, cfg.port))
}

fn cmd_status(cfg: &Config) -> Result<(), String> {
    if cfg.dev_id == "unknown" || cfg.dev_id.is_empty() {
        println!("C236T at {}:{}", cfg.ip, cfg.port);
    } else {
        println!("C236T {} at {}:{}", cfg.dev_id, cfg.ip, cfg.port);
    }
    let mut client = connect(cfg)?;
    if let Some(key) = client.session_key() {
        println!("session key: {}", hex::encode(key));
    }
    let dps = client
        .query_snapshot(Duration::from_millis(1200))
        .map_err(|e| format!("DP query failed: {e}"))?;
    println!("{} data points\n", dps.len());
    print_dps(&dps, None);
    client.close();
    Ok(())
}

fn cmd_get(cfg: &Config, key: Option<&String>) -> Result<(), String> {
    let key = key.ok_or_else(|| "usage: c236t-ctl get <dp-id|name>".to_string())?;
    let info = dp::dp_info(key);
    let id = info.map(|i| i.id.to_string()).unwrap_or_else(|| key.clone());

    let mut client = connect(cfg)?;
    let dps = client
        .query_snapshot(Duration::from_millis(1200))
        .map_err(|e| format!("DP query failed: {e}"))?;
    client.close();

    match dps.get(&id) {
        Some(value) => {
            let name = info.map(|i| i.name).unwrap_or("unknown");
            println!("{id} {name} = {}", dp::format_dp(info, value));
            if let Some(info) = info {
                println!("  {}", info.description);
            }
            Ok(())
        }
        None => Err(format!("device did not report DP {id} ({})", dp::dp_label(&id))),
    }
}

fn cmd_set(cfg: &Config, assignments: &[String]) -> Result<(), String> {
    if assignments.is_empty() {
        return Err("usage: c236t-ctl set <dp>=<value> [<dp>=<value> ...]".to_string());
    }

    let mut dps: HashMap<String, Value> = HashMap::new();
    for assignment in assignments {
        let (key, raw) = assignment
            .split_once('=')
            .ok_or_else(|| format!("expected <dp>=<value>, got `{assignment}`"))?;
        let info = dp::dp_info(key)
            .ok_or_else(|| format!("unknown DP `{key}` (run `c236t-ctl list`)"))?;
        let value = parse_value(info, raw)?;
        println!(
            "  {} = {} ({})",
            info.name,
            dp::format_dp(Some(info), &value),
            info.id
        );
        dps.insert(info.id.to_string(), value);
    }

    let mut client = connect(cfg)?;
    client
        .control_dps(&dps)
        .map_err(|e| format!("control failed: {e}"))?;
    println!("device acknowledged; re-reading state...");

    // Give the firmware a moment to apply the change, then confirm.
    std::thread::sleep(Duration::from_millis(300));
    let state = client
        .query_snapshot(Duration::from_millis(800))
        .map_err(|e| format!("DP query failed: {e}"))?;
    client.close();

    for id in dps.keys() {
        let info = dp::dp_info(id);
        match state.get(id) {
            Some(value) => println!("  now {} = {}", dp::dp_label(id), dp::format_dp(info, value)),
            None => println!("  now {} = (not reported)", dp::dp_label(id)),
        }
    }
    Ok(())
}

fn cmd_raw(cfg: &Config, arg: Option<&String>) -> Result<(), String> {
    let arg = arg.ok_or_else(|| "usage: c236t-ctl raw '<json>'".to_string())?;
    let value: Value = serde_json::from_str(arg).map_err(|e| format!("invalid JSON: {e}"))?;
    let mut client = connect(cfg)?;
    client
        .control_json(value)
        .map_err(|e| format!("control failed: {e}"))?;
    client.close();
    println!("device acknowledged");
    Ok(())
}

fn cmd_watch(cfg: &Config) -> Result<(), String> {
    loop {
        match C236tClient::connect(&cfg.ip, cfg.port, &cfg.local_key) {
            Ok(mut client) => {
                println!(
                    "[{}] connected, session key {}\n",
                    now(),
                    client
                        .session_key()
                        .map(hex::encode)
                        .unwrap_or_else(|| "?".to_string())
                );
                let mut unit_f = None;
                if let Ok(dps) = client.query_snapshot(Duration::from_millis(800)) {
                    if let Some(v) = dps.get("106").and_then(|v| v.as_bool()) {
                        unit_f = Some(v);
                    }
                    print_dps(&dps, unit_f);
                }
                println!("\nwatching for reports (Ctrl-C to stop)\n");

                let mut last_heartbeat = Instant::now();
                loop {
                    if last_heartbeat.elapsed() >= Duration::from_secs(10) {
                        if let Err(e) = client.send(protocol::CMD_HEARTBEAT, b"") {
                            eprintln!("[{}] heartbeat failed: {e}", now());
                            break;
                        }
                        last_heartbeat = Instant::now();
                    }

                    match client.recv_frame(Duration::from_secs(1)) {
                        Ok(frame) => match frame.cmd {
                            protocol::CMD_DP_REPORT => match frame.json() {
                                Ok(value) => match protocol::dps_from_json(value) {
                                    Ok(dps) => {
                                        if let Some(v) = dps.get("106").and_then(|v| v.as_bool()) {
                                            unit_f = Some(v);
                                        }
                                        println!("[{}] report ({} DP)", now(), dps.len());
                                        print_dps(&dps, unit_f);
                                    }
                                    Err(e) => eprintln!("[{}] bad report: {e}", now()),
                                },
                                Err(e) => eprintln!("[{}] bad report: {e}", now()),
                            },
                            protocol::CMD_HEARTBEAT => {}
                            other => println!("[{}] cmd {other}", now()),
                        },
                        Err(e) if protocol::is_timeout(&e) => {}
                        Err(e) => {
                            eprintln!("[{}] connection lost: {e}", now());
                            break;
                        }
                    }
                }
            }
            Err(e) => eprintln!("[{}] connect failed: {e}", now()),
        }
        eprintln!("reconnecting in 3s...\n");
        std::thread::sleep(Duration::from_secs(3));
    }
}

fn cmd_list() {
    println!(
        "{:<4} {:<22} {:<9} {}",
        "DP", "name", "type", "description"
    );
    for info in DP_TABLE {
        let kind = match info.kind {
            DpKind::Bool => "bool",
            DpKind::Int => "int",
            DpKind::Temp => "temp",
            DpKind::Text => "text",
            DpKind::Momentary => "trigger",
        };
        println!(
            "{:<4} {:<22} {:<9} {}",
            info.id, info.name, kind, info.description
        );
    }
}

fn print_dps(dps: &HashMap<String, Value>, unit_f: Option<bool>) {
    let mut entries: Vec<(&String, &Value)> = dps.iter().collect();
    entries.sort_by_key(|(id, _)| id.parse::<u32>().unwrap_or(u32::MAX));

    let unit = match unit_f.or_else(|| dps.get("106").and_then(|v| v.as_bool())) {
        Some(true) => "°F",
        Some(false) => "°C",
        None => "°",
    };

    for (id, value) in entries {
        let info = dp::dp_info(id);
        match info {
            Some(info) => {
                let mut formatted = dp::format_dp(Some(info), value);
                if info.kind == DpKind::Temp {
                    formatted.push_str(unit);
                }
                println!(
                    "  {:>3}  {:<22} {:<20} {}",
                    id, info.name, formatted, info.description
                );
            }
            None => println!("  {:>3}  {:<22} {:<20} (unknown DP)", id, "unknown", value),
        }
    }
}

fn parse_value(info: &DpInfo, raw: &str) -> Result<Value, String> {
    let raw = raw.trim();
    match info.kind {
        DpKind::Bool | DpKind::Momentary => match raw.to_ascii_lowercase().as_str() {
            "true" | "on" | "1" | "yes" => Ok(json!(true)),
            "false" | "off" | "0" | "no" => Ok(json!(false)),
            _ => Err(format!("`{raw}` is not a boolean (use on/off or true/false)")),
        },
        DpKind::Temp => {
            let degrees: f64 = raw
                .parse()
                .map_err(|_| format!("`{raw}` is not a number"))?;
            Ok(json!((degrees * 100.0).round() as i64))
        }
        DpKind::Int => {
            if let Ok(v) = raw.parse::<i64>() {
                Ok(json!(v))
            } else if let Ok(v) = raw.parse::<f64>() {
                Ok(json!(v.round() as i64))
            } else {
                Err(format!("`{raw}` is not an integer"))
            }
        }
        DpKind::Text => Ok(json!(raw)),
    }
}

fn now() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("{secs}")
}

// ---------------------------------------------------------------------------
// discover + setup
// ---------------------------------------------------------------------------

fn parse_timeout(args: &[String], default_secs: u64) -> Result<Duration, String> {
    let mut secs: Option<u64> = None;
    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        if let Some(v) = arg.strip_prefix("--timeout=") {
            secs = Some(parse_timeout_value(v)?);
        } else if arg == "--timeout" {
            i += 1;
            if i >= args.len() {
                return Err("usage: c236t-ctl discover [--timeout <secs>]".to_string());
            }
            secs = Some(parse_timeout_value(&args[i])?);
        } else if arg.starts_with("--") {
            return Err(format!(
                "unknown discover option `{arg}` (try `c236t-ctl discover --timeout 10`)"
            ));
        } else if secs.is_none() {
            secs = Some(parse_timeout_value(arg)?);
        } else {
            return Err(format!(
                "unexpected argument `{arg}` (try `c236t-ctl discover [--timeout <secs>]`)"
            ));
        }
        i += 1;
    }
    Ok(Duration::from_secs(secs.unwrap_or(default_secs)))
}

fn parse_timeout_value(v: &str) -> Result<u64, String> {
    let n: u64 = v
        .trim()
        .parse()
        .map_err(|_| format!("invalid timeout `{v}` (seconds)"))?;
    if n == 0 || n > 120 {
        return Err("timeout must be 1..=120 seconds".to_string());
    }
    Ok(n)
}

fn cmd_discover(args: &[String]) -> Result<(), String> {
    let timeout = parse_timeout(args, 8)?;
    let local_ip = discovery::local_ipv4().unwrap_or_else(|| "<unknown>".to_string());
    println!(
        "Scanning for Tuya devices on UDP 7000 for {}s (local IP {})...",
        timeout.as_secs(),
        local_ip
    );
    let (devices, warnings) = discovery::discover(timeout);
    for w in &warnings {
        eprintln!("warning: {w}");
    }
    if devices.is_empty() {
        println!("No devices found.");
        println!(
            "Hints: keep this host on the same WiFi/LAN as the device, allow UDP 7000 in the firewall,\n\
             wait for the device to be online, and try `c236t-ctl discover --timeout 15`."
        );
        return Ok(());
    }
    println!(
        "\n{:<16} {:<22} {:<18} {}",
        "IP", "ID (gwId)", "productKey", "version"
    );
    for d in &devices {
        let marker = if d.is_c236t() { " ✓ C236T" } else { "" };
        println!(
            "{:<16} {:<22} {:<18} {}{}",
            d.ip, d.id, d.product_key, d.version, marker
        );
        if let Some(uuid) = &d.uuid {
            println!("  uuid: {uuid}");
        }
    }
    println!("\nNext: c236t-ctl setup <ip>  (then enter the 16-byte local key)");
    Ok(())
}

#[derive(Default, Clone)]
struct SetupArgs {
    ip: Option<String>,
    port: Option<u16>,
    dev_id: Option<String>,
    key: Option<String>,
}

fn parse_setup_args(args: &[String]) -> Result<SetupArgs, String> {
    let mut out = SetupArgs::default();
    let mut positional: Vec<String> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        let (name, inline) = if let Some(eq) = arg.find('=') {
            if arg.starts_with("--") {
                (&arg[..eq], Some(&arg[eq + 1..]))
            } else {
                (arg.as_str(), None)
            }
        } else {
            (arg.as_str(), None)
        };
        match name {
            "--ip" | "--host" => {
                let v = take_value(args, &mut i, inline, name)?;
                out.ip = Some(v);
            }
            "--port" => {
                let v = take_value(args, &mut i, inline, name)?;
                out.port = Some(
                    v.parse()
                        .map_err(|_| format!("invalid port `{v}` (1..=65535)"))?,
                );
            }
            "--id" | "--dev-id" | "--devid" | "--device-id" => {
                let v = take_value(args, &mut i, inline, name)?;
                out.dev_id = Some(v);
            }
            "--key" | "--local-key" | "--localkey" => {
                let v = take_value(args, &mut i, inline, name)?;
                out.key = Some(v);
            }
            "--help" | "-h" => return Err("usage: c236t-ctl setup [--ip <ip>] [--port <port>] [--id <devId>] [--key <localKey>]".to_string()),
            other if other.starts_with("--") => {
                return Err(format!("unknown setup option `{other}`"));
            }
            _ => positional.push(arg.clone()),
        }
        i += 1;
    }
    if !positional.is_empty() {
        out.ip.get_or_insert(positional[0].clone());
    }
    if positional.len() > 1 {
        if out.port.is_none() {
            out.port = Some(
                positional[1]
                    .parse()
                    .map_err(|_| format!("invalid port `{}` (1..=65535)", positional[1]))?,
            );
        }
    }
    if positional.len() > 2 {
        out.dev_id.get_or_insert(positional[2].clone());
    }
    if positional.len() > 3 {
        out.key.get_or_insert(positional[3].clone());
    }
    if positional.len() > 4 {
        return Err(format!("too many arguments (got `{}`)", positional[4]));
    }
    Ok(out)
}

fn take_value(args: &[String], i: &mut usize, inline: Option<&str>, name: &str) -> Result<String, String> {
    if let Some(v) = inline {
        return Ok(v.to_string());
    }
    *i += 1;
    if *i >= args.len() {
        return Err(format!("missing value for {name}"));
    }
    Ok(args[*i].clone())
}

fn prompt(label: &str, default: Option<&str>) -> Result<String, String> {
    let mut stdout = std::io::stdout();
    let hint = default.map(|d| format!(" [{d}]")).unwrap_or_default();
    print!("{label}{hint}: ");
    stdout.flush().map_err(|e| e.to_string())?;
    let stdin = std::io::stdin();
    let mut line = String::new();
    stdin
        .lock()
        .read_line(&mut line)
        .map_err(|e| e.to_string())?;
    let value = line.trim_end_matches(['\r', '\n']).to_string();
    if value.trim().is_empty() {
        Ok(default.unwrap_or("").to_string())
    } else {
        Ok(value)
    }
}

fn cmd_setup(args: &[String]) -> Result<(), String> {
    let parsed = parse_setup_args(args)?;
    let stdin_is_tty = unsafe {
        // libc::isatty on stdin; missing value means we cannot prompt.
        libc::isatty(libc::STDIN_FILENO) != 0
    };
    let require = |label: &str, flag: &str, value: Option<String>| -> Result<String, String> {
        match value {
            Some(v) if !v.trim().is_empty() => Ok(v.trim().to_string()),
            _ if stdin_is_tty => prompt(label, None).and_then(|v| {
                if v.trim().is_empty() {
                    Err(format!("{label} is required"))
                } else {
                    Ok(v.trim().to_string())
                }
            }),
            _ => Err(format!(
                "{label} is required (pass {flag} or run setup interactively)"
            )),
        }
    };

    // Defaults for prompts: env (shell/.env) > existing TOML > built-ins.
    let existing_cfg = file_config::load_file_config()
        .ok()
        .flatten()
        .unwrap_or_default();
    let ip_default = std::env::var("THING_LOCAL_IP").ok().or(existing_cfg.ip.clone());
    let port_default = std::env::var("THING_LOCAL_PORT")
        .ok()
        .or_else(|| existing_cfg.port.map(|p| p.to_string()));

    let ip = match parsed.ip {
        Some(v) => v.trim().to_string(),
        None if stdin_is_tty => prompt("Device IP", ip_default.as_deref())?,
        None => ip_default.ok_or_else(|| "device IP is required (pass --ip)".to_string())?,
    };
    if ip.trim().is_empty() {
        return Err("device IP is required".to_string());
    }
    let port = match parsed.port {
        Some(p) => p,
        None if stdin_is_tty => {
            let raw = prompt("Port", port_default.as_deref().or(Some("6668")))?;
            raw.trim()
                .parse()
                .map_err(|_| format!("invalid port `{raw}` (1..=65535)"))?
        }
        None => port_default
            .and_then(|p| p.parse().ok())
            .unwrap_or(6668),
    };
    if port == 0 {
        return Err("invalid port `0` (1..=65535)".to_string());
    }
    // Device ID is informational only — never prompt for it.
    let dev_id = parsed
        .dev_id
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .or_else(|| existing_cfg.device_id.clone())
        .unwrap_or_else(|| "unknown".to_string());
    let key = require("local key", "--key", parsed.key)?;
    if key.as_bytes().len() != 16 {
        return Err(format!(
            "local key must be exactly 16 bytes, got {}",
            key.as_bytes().len()
        ));
    }

    if dev_id == "unknown" {
        println!("Verifying at {ip}:{port}...");
    } else {
        println!("Verifying {dev_id} at {ip}:{port}...");
    }
    let cfg = Config {
        ip: ip.clone(),
        port,
        local_key: key.clone().into_bytes(),
        dev_id: dev_id.clone(),
    };
    let mut client = connect(&cfg).map_err(|e| {
        format!(
            "{e}\ncheck the IP/port and local key; if the official app holds the session, force-stop it first"
        )
    })?;
    let dps = client
        .query_snapshot(Duration::from_millis(800))
        .map_err(|e| format!("DP query failed: {e}"))?;
    client.close();
    println!("verified: device replied with {} data points", dps.len());
    if let Some(v) = dps.get("129") {
        if let Some(raw) = v.as_i64() {
            println!("current temperature: {:.2} (DP 129)", raw as f64 / 100.0);
        }
    }

    let cfg_path = file_config::canonical_path();
    let fc = file_config::FileConfig {
        local_key: Some(key.clone()),
        ip: Some(ip.clone()),
        port: Some(port),
        device_id: if dev_id == "unknown" { None } else { Some(dev_id.clone()) },
    };
    file_config::save_file_config(&cfg_path, &fc)?;
    // Reflect into this process env for consistency.
    std::env::set_var("THING_LOCAL_IP", &ip);
    std::env::set_var("THING_LOCAL_PORT", port.to_string());
    if dev_id != "unknown" {
        std::env::set_var("THING_DEV_ID", &dev_id);
    }
    std::env::set_var("THING_LOCAL_KEY", &key);
    println!("saved to {} — run `c236t-ctl status` to check", cfg_path.display());
    Ok(())
}

fn usage() {
    println!(
        "C236T local controller (Tuya protocol 3.5, AES-GCM)\n\
         \n\
         USAGE:\n\
           c236t-ctl status                 Read and print every data point\n\
           c236t-ctl get <dp>               Read one data point\n\
           c236t-ctl set <dp>=<value> ...   Write one or more data points\n\
           c236t-ctl watch                  Stream live reports (auto-reconnect)\n\
           c236t-ctl raw '<json>'           Send a raw dps object or control JSON\n\
           c236t-ctl list                   List all known data points\n\
           c236t-ctl discover [--timeout <secs>]  Find Tuya devices on the LAN (UDP 7000)\n\
           c236t-ctl setup [--ip <ip>] [--port <port>] [--key <localKey>]\n\
                                Interactive setup; verifies the handshake, then writes c236t.toml\n\
                                (--id is accepted but optional; never prompted)\n\
         \n\
         <dp> is a numeric DP id or a name from `c236t-ctl list`.\n\
         Temperatures are entered in degrees (e.g. 25 or 25.5).\n\
         Switches accept on/off, true/false, 1/0.\n\
         \n\
         Configuration (precedence: shell env > c236t.toml > .env):\n\
           c236t.toml          local_key (required, 16 bytes), ip (required), port, device_id\n\
           C236T_CONFIG_FILE   explicit config file path\n\
           THING_LOCAL_KEY     device local key (shell env override)\n\
           THING_LOCAL_IP      device LAN address (required)\n\
           THING_LOCAL_PORT    device LAN port (default 6668)"
    );
}
