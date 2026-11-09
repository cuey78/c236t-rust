/*
 * UDP discovery for Tuya 3.5 devices (cmd = REQ_DEVINFO on port 7000).
 *
 * The broadcast is encrypted with the shared `UDP_KEY` and devices answer
 * with a devinfo JSON (`ip`, `gwId`, `productKey`, ...). The local key is
 * never included in discovery responses.
 */

use crate::protocol::{build_discovery_packet, decrypt_udp_json, UDP_PORT_6666, UDP_PORT_6667, UDP_PORT_DISCOVERY};
use std::collections::HashMap;
use std::net::UdpSocket;
use std::time::{Duration, Instant};

/// One discovered Tuya device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredDevice {
    pub ip: String,
    pub id: String,
    pub product_key: String,
    pub version: String,
    pub uuid: Option<String>,
}

impl DiscoveredDevice {
    pub fn is_c236t(&self) -> bool {
        self.product_key == "ae6rmrnkqz7rjh66"
    }
}

/// Best-effort local IPv4 address via the UDP-connect trick (no traffic sent).
pub fn local_ipv4() -> Option<String> {
    for target in ["8.8.8.8:80", "1.1.1.1:80"] {
        if let Ok(sock) = UdpSocket::bind("0.0.0.0:0") {
            if sock.connect(target).is_ok() {
                return sock.local_addr().ok().map(|a| a.ip().to_string());
            }
        }
    }
    None
}

/// Broadcast targets for `local_ip`: global broadcast plus its `.255` subnet broadcast.
pub fn broadcast_targets(local_ip: &str) -> Vec<String> {
    let mut targets = vec!["255.255.255.255".to_string()];
    let parts: Vec<&str> = local_ip.split('.').collect();
    if parts.len() == 4 {
        let last: u8 = parts[3].parse().unwrap_or(0);
        if last != 255 {
            targets.push(format!("{}.{}.{}.255", parts[0], parts[1], parts[2]));
        }
    }
    targets
}

fn parse_device(value: &serde_json::Value, fallback_ip: &str) -> Option<DiscoveredDevice> {
    let obj = value.as_object()?;
    // Own broadcast echo carries `from == "app"`.
    if obj.get("from").and_then(|v| v.as_str()) == Some("app") {
        return None;
    }
    let id = obj
        .get("gwId")
        .or_else(|| obj.get("id"))
        .and_then(|v| v.as_str())?;
    let ip = obj
        .get("ip")
        .and_then(|v| v.as_str())
        .unwrap_or(fallback_ip)
        .to_string();
    Some(DiscoveredDevice {
        ip,
        id: id.to_string(),
        product_key: obj
            .get("productKey")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        version: obj
            .get("version")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        uuid: obj
            .get("uuid")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
    })
}

/// Scan the LAN for Tuya devices. Returns `(devices, warnings)`; `warnings`
/// reports non-fatal listener problems.
pub fn discover(timeout: Duration) -> (Vec<DiscoveredDevice>, Vec<String>) {
    let local_ip = match local_ipv4() {
        Some(ip) => ip,
        None => return (Vec::new(), vec!["could not determine local IP address".to_string()]),
    };
    let targets = broadcast_targets(&local_ip);

    let mut warnings = Vec::new();
    let mut listeners: Vec<UdpSocket> = Vec::new();
    for port in [UDP_PORT_6666, UDP_PORT_6667, UDP_PORT_DISCOVERY] {
        match UdpSocket::bind(("0.0.0.0", port)) {
            Ok(sock) => {
                sock.set_broadcast(true).ok();
                sock.set_nonblocking(true).ok();
                listeners.push(sock);
            }
            Err(e) => warnings.push(format!("could not listen on UDP {port}: {e}")),
        }
    }
    if listeners.is_empty() {
        warnings.push("no UDP listeners could be opened".to_string());
        return (Vec::new(), warnings);
    }
    if warnings
        .iter()
        .any(|w| w.contains(&UDP_PORT_DISCOVERY.to_string()))
    {
        warnings.push(
            "port 7000 could not be opened; 3.5 devices will not be found".to_string(),
        );
    }

    let sender = UdpSocket::bind((local_ip.as_str(), 0))
        .or_else(|_| UdpSocket::bind("0.0.0.0:0"));
    let sender = match sender {
        Ok(s) => {
            s.set_broadcast(true).ok();
            s
        }
        Err(e) => {
            warnings.push(format!("could not open broadcast socket: {e}"));
            return (Vec::new(), warnings);
        }
    };

    let packet = match build_discovery_packet(&local_ip) {
        Ok(p) => p,
        Err(e) => {
            warnings.push(format!("could not build discovery packet: {e}"));
            return (Vec::new(), warnings);
        }
    };

    let mut send_all = |sender: &UdpSocket| {
        for target in &targets {
            if let Err(e) = sender.send_to(&packet, (target.as_str(), UDP_PORT_DISCOVERY)) {
                warnings.push(format!("broadcast to {target} failed: {e}"));
            }
        }
    };
    send_all(&sender);

    let deadline = Instant::now() + timeout;
    let mut last_send = Instant::now();
    let mut found: HashMap<String, DiscoveredDevice> = HashMap::new();
    let mut buf = [0u8; 4096];

    while Instant::now() < deadline {
        if last_send.elapsed() >= Duration::from_secs(2) {
            send_all(&sender);
            last_send = Instant::now();
        }
        for sock in &listeners {
            loop {
                match sock.recv_from(&mut buf) {
                    Ok((n, addr)) => {
                        if n == 0 {
                            continue;
                        }
                        if let Ok(value) = decrypt_udp_json(&buf[..n]) {
                            if let Some(dev) = parse_device(&value, &addr.ip().to_string()) {
                                found
                                    .entry(format!("{}:{}", dev.id, dev.ip))
                                    .or_insert(dev);
                            }
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(_) => break,
                }
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }

    let mut devices: Vec<DiscoveredDevice> = found.into_values().collect();
    devices.sort_by(|a, b| a.ip.cmp(&b.ip).then(a.id.cmp(&b.id)));
    (devices, warnings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn broadcast_targets_cover_subnet() {
        assert_eq!(broadcast_targets("192.168.1.2")[0], "255.255.255.255");
        assert_eq!(broadcast_targets("192.168.1.2")[1], "192.168.1.255");
        assert_eq!(broadcast_targets("192.168.1.2")[1], "192.168.1.255");
    }

    #[test]
    fn skips_app_echo() {
        let echo: serde_json::Value =
            serde_json::from_str(r#"{"from":"app","ip":"192.168.1.2"}"#).unwrap();
        assert_eq!(parse_device(&echo, "192.168.1.2"), None);
    }

    #[test]
    fn parses_devinfo() {
        let info: serde_json::Value = serde_json::from_str(
            r#"{"ip":"192.168.1.100","gwId":"example-device-id","productKey":"ae6rmrnkqz7rjh66","version":"3.5","uuid":"example-uuid-0123456789"}"#,
        )
        .unwrap();
        let dev = parse_device(&info, "192.168.1.100").unwrap();
        assert_eq!(dev.id, "example-device-id");
        assert!(dev.is_c236t());
        assert_eq!(dev.version, "3.5");
    }
}
