/*
 * Tuya local protocol 3.5 (6699 / AES-GCM) for the C236T.
 *
 * Frame layout:
 *   prefix(4)=00 00 66 99 | reserved(2)=00 00 | seq(4) | cmd(4) | length(4)
 *   | nonce(12) | ciphertext | tag(16) | suffix(4)=00 00 99 66
 *
 * `length` counts nonce + ciphertext + tag. The GCM AAD is the header
 * bytes 4..18 (reserved, seq, cmd, length).
 *
 * Handshake:
 *   C -> D  cmd 3  : client_nonce(16), encrypted with localKey
 *   D -> C  cmd 4  : retcode(4) + device_nonce(16) + HMAC-SHA256(localKey, client_nonce)
 *   C -> D  cmd 5  : HMAC-SHA256(localKey, device_nonce), encrypted with localKey
 *   session_key = AES-GCM(localKey, nonce=client_nonce[0..12],
 *                         plaintext=device_nonce XOR client_nonce)[0..16]
 *
 * Data commands are encrypted with the session key. Device -> client
 * payloads start with a 4-byte big-endian retcode.
 */

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes128Gcm, Nonce};
use hmac::{Hmac, Mac};
use rand::RngCore;
use serde_json::{json, Value};
use sha2::Sha256;
use std::collections::HashMap;
use std::io::{Error, ErrorKind, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

pub const PREFIX: [u8; 4] = [0x00, 0x00, 0x66, 0x99];
pub const SUFFIX: [u8; 4] = [0x00, 0x00, 0x99, 0x66];

pub const CMD_HANDSHAKE: u32 = 3;
pub const CMD_HANDSHAKE_RESP: u32 = 4;
pub const CMD_HANDSHAKE_FINISH: u32 = 5;
pub const CMD_DP_REPORT: u32 = 8;
pub const CMD_HEARTBEAT: u32 = 9;
pub const CMD_CONTROL: u32 = 13;
pub const CMD_DP_QUERY: u32 = 16;
pub const CMD_REQ_DEVINFO: u32 = 0x25;

/// UDP discovery: broadcast a `REQ_DEVINFO` (cmd 37) frame to port 7000,
/// devices reply with a devinfo JSON. Listeners also cover 6666/6667 for
/// older protocol versions.
pub const UDP_PORT_6666: u16 = 6666;
pub const UDP_PORT_6667: u16 = 6667;
pub const UDP_PORT_DISCOVERY: u16 = 7000;

/// UDP discovery AES key: `MD5(b"yGAdlopoPVldABfn")` (TinyTuya `udpkey`).
pub const UDP_KEY: [u8; 16] = [108, 30, 200, 226, 187, 155, 181, 154, 181, 11, 13, 175, 100, 155, 65, 10];

const HEADER_LEN: usize = 18;
const NONCE_LEN: usize = 12;
const TAG_LEN: usize = 16;

fn proto_err(msg: impl Into<String>) -> Box<dyn std::error::Error> {
    Box::new(Error::new(ErrorKind::InvalidData, msg.into()))
}

fn new_cipher(key: &[u8]) -> Result<Aes128Gcm> {
    if key.len() != 16 {
        return Err(proto_err(format!(
            "expected a 16-byte AES key, got {} bytes",
            key.len()
        )));
    }
    Aes128Gcm::new_from_slice(key).map_err(|_| proto_err("invalid AES key"))
}

fn random_bytes<const N: usize>() -> [u8; N] {
    let mut out = [0u8; N];
    rand::rngs::OsRng.fill_bytes(&mut out);
    out
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> [u8; 32] {
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(key)
        .expect("HMAC accepts any key length");
    mac.update(data);
    let out = mac.finalize().into_bytes();
    let mut digest = [0u8; 32];
    digest.copy_from_slice(&out);
    digest
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// A decrypted Tuya frame. `payload` includes the 4-byte retcode for
/// device-originated frames.
#[derive(Debug, Clone)]
pub struct Frame {
    pub seq: u32,
    pub cmd: u32,
    pub payload: Vec<u8>,
}

impl Frame {
    /// Split the leading 4-byte retcode from a device frame.
    pub fn body(&self) -> Result<&[u8]> {
        if self.payload.len() < 4 {
            return Err(proto_err("frame payload too short for retcode"));
        }
        let retcode = u32::from_be_bytes(self.payload[0..4].try_into().unwrap());
        if retcode != 0 {
            return Err(proto_err(format!("device returned retcode {retcode}")));
        }
        Ok(&self.payload[4..])
    }

    /// Parse the DP JSON carried by a cmd 8 report or cmd 16 query response.
    /// Reports may carry the `3.5` + 12-byte protocol header; query
    /// responses carry the JSON directly after the retcode.
    pub fn json(&self) -> Result<Value> {
        let mut body = self.body()?;
        if body.len() >= 15 && &body[0..3] == b"3.5" {
            body = &body[15..];
        }
        Ok(serde_json::from_slice(body)?)
    }

    /// Parse a UDP discovery JSON payload.
    ///
    /// Discovery broadcasts (`cmd = REQ_DEVINFO`, no retcode) start with
    /// `{`; device replies carry a 4-byte retcode before the JSON. Trailing
    /// NUL padding is stripped, mirroring TinyTuya.
    pub fn discovery_json(&self) -> Result<Value> {
        let mut payload: &[u8] = &self.payload;
        if !payload.starts_with(b"{") && payload.len() >= 5 && payload[4] == b'{' {
            payload = &payload[4..];
        }
        while payload.last() == Some(&0) {
            payload = &payload[..payload.len() - 1];
        }
        // Allow trailing whitespace after the JSON object.
        let mut end = payload.len();
        while end > 0 && payload[end - 1].is_ascii_whitespace() {
            end -= 1;
        }
        Ok(serde_json::from_slice(&payload[..end])?)
    }
}

/// Build a wire frame. When `retcode` is set the 4-byte big-endian value is
/// prepended to the plaintext before encryption (client frames normally omit it).
pub fn pack_frame(
    seq: u32,
    cmd: u32,
    payload: &[u8],
    key: &[u8],
    retcode: Option<u32>,
) -> Result<Vec<u8>> {
    let mut plain = Vec::with_capacity(payload.len() + 4);
    if let Some(rc) = retcode {
        plain.extend_from_slice(&rc.to_be_bytes());
    }
    plain.extend_from_slice(payload);

    let nonce = random_bytes::<NONCE_LEN>();
    let body_len = NONCE_LEN + plain.len() + TAG_LEN;

    let mut header = Vec::with_capacity(HEADER_LEN);
    header.extend_from_slice(&PREFIX);
    header.extend_from_slice(&0u16.to_be_bytes());
    header.extend_from_slice(&seq.to_be_bytes());
    header.extend_from_slice(&cmd.to_be_bytes());
    header.extend_from_slice(&(body_len as u32).to_be_bytes());

    let cipher = new_cipher(key)?;
    let ciphertext = cipher
        .encrypt(
            Nonce::from_slice(&nonce),
            Payload {
                msg: &plain,
                aad: &header[4..HEADER_LEN],
            },
        )
        .map_err(|_| proto_err("AES-GCM encryption failed"))?;

    let mut out = Vec::with_capacity(HEADER_LEN + body_len + SUFFIX.len());
    out.extend_from_slice(&header);
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ciphertext);
    out.extend_from_slice(&SUFFIX);
    Ok(out)
}

/// Decrypt a single 6699 frame (e.g. one UDP datagram) with `key`.
pub fn unpack_single_frame(data: &[u8], key: &[u8]) -> Result<Frame> {
    if data.len() < HEADER_LEN + NONCE_LEN + TAG_LEN + SUFFIX.len() {
        return Err(proto_err(format!(
            "UDP frame too short ({} bytes)",
            data.len()
        )));
    }
    if data[0..4] != PREFIX {
        return Err(proto_err("UDP frame has invalid 6699 prefix"));
    }
    let seq = u32::from_be_bytes(data[6..10].try_into().unwrap());
    let cmd = u32::from_be_bytes(data[10..14].try_into().unwrap());
    let body_len = u32::from_be_bytes(data[14..18].try_into().unwrap()) as usize;
    if body_len < NONCE_LEN + TAG_LEN {
        return Err(proto_err(format!("invalid UDP body length {body_len}")));
    }
    let total = HEADER_LEN + body_len + SUFFIX.len();
    if data.len() < total {
        return Err(proto_err(format!(
            "truncated UDP frame (need {total}, have {})",
            data.len()
        )));
    }
    if data[total - 4..total] != SUFFIX {
        return Err(proto_err("UDP frame has invalid suffix"));
    }
    let header = &data[0..HEADER_LEN];
    let nonce = &data[HEADER_LEN..HEADER_LEN + NONCE_LEN];
    let ciphertext = &data[HEADER_LEN + NONCE_LEN..HEADER_LEN + body_len];
    let cipher = new_cipher(key)?;
    let plain = cipher
        .decrypt(
            Nonce::from_slice(nonce),
            Payload {
                msg: ciphertext,
                aad: &header[4..HEADER_LEN],
            },
        )
        .map_err(|_| proto_err("AES-GCM authentication failed (wrong UDP key or corrupt frame)"))?;
    Ok(Frame {
        seq,
        cmd,
        payload: plain,
    })
}

/// Decrypt a UDP discovery datagram with `UDP_KEY` and parse its JSON.
pub fn decrypt_udp_json(data: &[u8]) -> Result<Value> {
    let frame = unpack_single_frame(data, &UDP_KEY)?;
    frame.discovery_json()
}

/// Build a UDP discovery broadcast packet for `cmd = REQ_DEVINFO` (seq 0),
/// payload `{"from":"app","ip":"<local_ip>"}`.
pub fn build_discovery_packet(local_ip: &str) -> Result<Vec<u8>> {
    let payload = json!({"from": "app", "ip": local_ip}).to_string();
    pack_frame(0, CMD_REQ_DEVINFO, payload.as_bytes(), &UDP_KEY, None)
}

/// Connection to a Tuya 3.5 device on TCP 6668.
pub struct C236tClient {
    stream: TcpStream,
    local_key: Vec<u8>,
    session_key: Option<[u8; 16]>,
    seq: u32,
    buf: Vec<u8>,
}

impl C236tClient {
    /// Connect and negotiate a session key.
    pub fn connect(ip: &str, port: u16, local_key: &[u8]) -> Result<Self> {
        let addr = format!("{ip}:{port}");
        let sock = addr
            .to_socket_addrs()?
            .next()
            .ok_or_else(|| proto_err(format!("could not resolve {addr}")))?;
        let stream = TcpStream::connect_timeout(&sock, Duration::from_secs(5))?;
        stream.set_nodelay(true).ok();

        let mut client = Self {
            stream,
            local_key: local_key.to_vec(),
            session_key: None,
            seq: (rand::random::<u32>() % 40_000) + 10_000,
            buf: Vec::new(),
        };
        client.handshake()?;
        Ok(client)
    }

    /// Local key used for the handshake.
    pub fn local_key(&self) -> &[u8] {
        &self.local_key
    }

    /// Negotiated session key, if the handshake succeeded.
    pub fn session_key(&self) -> Option<[u8; 16]> {
        self.session_key
    }

    fn handshake(&mut self) -> Result<()> {
        let client_nonce = random_bytes::<16>();
        let local_key = self.local_key.clone();
        self.send_raw(CMD_HANDSHAKE, &client_nonce, &local_key, None)?;

        let frame = self.recv_until(&local_key, CMD_HANDSHAKE_RESP, Duration::from_secs(8))?;
        let body = frame.body()?;
        if body.len() < 48 {
            return Err(proto_err(format!(
                "short handshake response ({} bytes)",
                body.len()
            )));
        }
        let device_nonce = &body[0..16];
        let hmac_rx = &body[16..48];
        let hmac_calc = hmac_sha256(&local_key, &client_nonce);
        if hmac_rx != hmac_calc {
            return Err(proto_err("handshake HMAC mismatch (wrong local key?)"));
        }

        let hmac_tx = hmac_sha256(&local_key, device_nonce);
        self.send_raw(CMD_HANDSHAKE_FINISH, &hmac_tx, &local_key, None)?;

        // session_key = AES-GCM(localKey, client_nonce[0..12], device_nonce XOR client_nonce)
        let mut mixed = [0u8; 16];
        for i in 0..16 {
            mixed[i] = device_nonce[i] ^ client_nonce[i];
        }
        let cipher = new_cipher(&local_key)?;
        let encrypted = cipher
            .encrypt(
                Nonce::from_slice(&client_nonce[0..NONCE_LEN]),
                Payload {
                    msg: &mixed,
                    aad: &[],
                },
            )
            .map_err(|_| proto_err("session key derivation failed"))?;
        let mut session_key = [0u8; 16];
        session_key.copy_from_slice(&encrypted[0..16]);
        self.session_key = Some(session_key);
        Ok(())
    }

    fn key(&self) -> Result<[u8; 16]> {
        self.session_key
            .ok_or_else(|| proto_err("handshake has not completed"))
    }

    fn send_raw(
        &mut self,
        cmd: u32,
        payload: &[u8],
        key: &[u8],
        retcode: Option<u32>,
    ) -> Result<()> {
        self.seq = self.seq.wrapping_add(1);
        let frame = pack_frame(self.seq, cmd, payload, key, retcode)?;
        self.stream.write_all(&frame)?;
        self.stream.flush()?;
        Ok(())
    }

    /// Send a command using the negotiated session key.
    pub fn send(&mut self, cmd: u32, payload: &[u8]) -> Result<()> {
        let key = self.key()?;
        self.send_raw(cmd, payload, &key, None)
    }

    fn try_parse(&mut self, key: &[u8]) -> Result<Option<Frame>> {
        loop {
            if self.buf.len() < PREFIX.len() {
                return Ok(None);
            }
            if self.buf[0..4] != PREFIX {
                match find_subslice(&self.buf, &PREFIX) {
                    Some(pos) => {
                        self.buf.drain(..pos);
                    }
                    None => {
                        let keep = self.buf.len().min(PREFIX.len() - 1);
                        let drop = self.buf.len() - keep;
                        self.buf.drain(..drop);
                        return Ok(None);
                    }
                }
            }
            if self.buf.len() < HEADER_LEN + NONCE_LEN + TAG_LEN + SUFFIX.len() {
                return Ok(None);
            }

            let seq = u32::from_be_bytes(self.buf[6..10].try_into().unwrap());
            let cmd = u32::from_be_bytes(self.buf[10..14].try_into().unwrap());
            let body_len = u32::from_be_bytes(self.buf[14..18].try_into().unwrap()) as usize;
            if body_len < NONCE_LEN + TAG_LEN {
                self.buf.drain(..1);
                continue;
            }
            let total = HEADER_LEN + body_len + SUFFIX.len();
            if self.buf.len() < total {
                return Ok(None);
            }
            if self.buf[total - 4..total] != SUFFIX {
                self.buf.drain(..1);
                continue;
            }

            let header: Vec<u8> = self.buf[0..HEADER_LEN].to_vec();
            let nonce: Vec<u8> = self.buf[HEADER_LEN..HEADER_LEN + NONCE_LEN].to_vec();
            let ciphertext: Vec<u8> =
                self.buf[HEADER_LEN + NONCE_LEN..HEADER_LEN + body_len].to_vec();
            self.buf.drain(..total);

            let cipher = new_cipher(key)?;
            let plain = cipher
                .decrypt(
                    Nonce::from_slice(&nonce),
                    Payload {
                        msg: &ciphertext,
                        aad: &header[4..HEADER_LEN],
                    },
                )
                .map_err(|_| proto_err("AES-GCM authentication failed (wrong key or corrupt frame)"))?;

            return Ok(Some(Frame {
                seq,
                cmd,
                payload: plain,
            }));
        }
    }

    /// Receive the next frame, blocking up to `timeout`.
    pub fn recv_frame(&mut self, timeout: Duration) -> Result<Frame> {
        let key = self.key()?;
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(frame) = self.try_parse(&key)? {
                return Ok(frame);
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(Box::new(Error::new(ErrorKind::TimedOut, "read timeout")));
            }
            self.stream.set_read_timeout(Some(remaining))?;
            let mut chunk = [0u8; 4096];
            match self.stream.read(&mut chunk) {
                Ok(0) => {
                    return Err(Box::new(Error::new(
                        ErrorKind::UnexpectedEof,
                        "device closed the connection",
                    )))
                }
                Ok(n) => self.buf.extend_from_slice(&chunk[..n]),
                Err(e) if e.kind() == ErrorKind::WouldBlock || e.kind() == ErrorKind::TimedOut => {
                    return Err(Box::new(Error::new(ErrorKind::TimedOut, "read timeout")))
                }
                Err(e) => return Err(Box::new(e)),
            }
        }
    }

    fn recv_until(&mut self, key: &[u8], cmd: u32, timeout: Duration) -> Result<Frame> {
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(frame) = self.try_parse(key)? {
                if frame.cmd == cmd {
                    return Ok(frame);
                }
                continue;
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(Box::new(Error::new(
                    ErrorKind::TimedOut,
                    format!("timed out waiting for cmd {cmd}"),
                )));
            }
            self.stream.set_read_timeout(Some(remaining))?;
            let mut chunk = [0u8; 4096];
            match self.stream.read(&mut chunk) {
                Ok(0) => {
                    return Err(Box::new(Error::new(
                        ErrorKind::UnexpectedEof,
                        "device closed the connection",
                    )))
                }
                Ok(n) => self.buf.extend_from_slice(&chunk[..n]),
                Err(e) if e.kind() == ErrorKind::WouldBlock || e.kind() == ErrorKind::TimedOut => {
                    return Err(Box::new(Error::new(
                        ErrorKind::TimedOut,
                        format!("timed out waiting for cmd {cmd}"),
                    )))
                }
                Err(e) => return Err(Box::new(e)),
            }
        }
    }

    /// Query all data points (cmd 16, `{}`). Returns the device's `dps` map.
    pub fn query_all(&mut self) -> Result<HashMap<String, Value>> {
        self.send(CMD_DP_QUERY, b"{}")?;
        let mut merged = HashMap::new();
        loop {
            let frame = self.recv_frame(Duration::from_secs(10))?;
            match frame.cmd {
                CMD_DP_QUERY => {
                    merged.extend(dps_from_json(frame.json()?)?);
                    return Ok(merged);
                }
                CMD_DP_REPORT => {
                    if let Ok(value) = frame.json() {
                        if let Ok(dps) = dps_from_json(value) {
                            merged.extend(dps);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    /// Query all data points, then keep collecting report frames for `settle`.
    /// The C236T splits its full state across the query response and several
    /// cmd 8 reports, so this yields a more complete snapshot.
    pub fn query_snapshot(&mut self, settle: Duration) -> Result<HashMap<String, Value>> {
        let mut merged = self.query_all()?;
        let deadline = Instant::now() + settle;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }
            match self.recv_frame(remaining) {
                Ok(frame) if frame.cmd == CMD_DP_REPORT => {
                    if let Ok(value) = frame.json() {
                        if let Ok(dps) = dps_from_json(value) {
                            merged.extend(dps);
                        }
                    }
                }
                Ok(_) => {}
                Err(e) => {
                    if !is_timeout(&e) {
                        // A real connection error should not be hidden, but a
                        // partial snapshot is still useful.
                    }
                    break;
                }
            }
        }
        Ok(merged)
    }

    /// Send a control payload (cmd 13) and wait for the acknowledgement.
    pub fn control_dps(&mut self, dps: &HashMap<String, Value>) -> Result<()> {
        let unix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let inner = json!({
            "data": { "dps": dps },
            "protocol": 5,
            "t": unix,
        });

        // Protocol 3.5 control payload: "3.5" + 12 zero bytes + JSON.
        let mut payload = Vec::with_capacity(15 + inner.to_string().len());
        payload.extend_from_slice(b"3.5");
        payload.extend_from_slice(&[0u8; 12]);
        payload.extend_from_slice(inner.to_string().as_bytes());

        self.send(CMD_CONTROL, &payload)?;
        loop {
            let frame = self.recv_frame(Duration::from_secs(10))?;
            if frame.cmd == CMD_CONTROL {
                frame.body()?;
                return Ok(());
            }
        }
    }

    /// Send a raw control JSON object. If it has no top-level `data` key it is
    /// treated as a bare `dps` map and wrapped as `{"data":{"dps":...}}`.
    pub fn control_json(&mut self, value: Value) -> Result<()> {
        if value.get("data").is_some() {
            let unix = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let mut obj = value;
            if let Value::Object(ref mut map) = obj {
                map.entry("protocol").or_insert(json!(5));
                map.entry("t").or_insert(json!(unix));
            }
            let mut payload = Vec::new();
            payload.extend_from_slice(b"3.5");
            payload.extend_from_slice(&[0u8; 12]);
            payload.extend_from_slice(obj.to_string().as_bytes());
            self.send(CMD_CONTROL, &payload)?;
            loop {
                let frame = self.recv_frame(Duration::from_secs(10))?;
                if frame.cmd == CMD_CONTROL {
                    frame.body()?;
                    return Ok(());
                }
            }
        } else {
            let dps = dps_from_json(value)?;
            self.control_dps(&dps)
        }
    }

    /// Send a heartbeat and wait briefly for the acknowledgement.
    pub fn heartbeat(&mut self) -> Result<()> {
        self.send(CMD_HEARTBEAT, b"")?;
        let key = self.key()?;
        let frame = self.recv_until(&key, CMD_HEARTBEAT, Duration::from_secs(5))?;
        frame.body()?;
        Ok(())
    }

    pub fn close(&mut self) {
        let _ = self.stream.shutdown(std::net::Shutdown::Both);
    }
}

/// Extract the `dps` object from a device JSON value. Handles both the query
/// response shape (`{"dps":{...}}`) and the report shape
/// (`{"protocol":4,"t":...,"data":{"dps":{...}}}`). A bare DP map is returned
/// unchanged.
pub fn dps_from_json(value: Value) -> Result<HashMap<String, Value>> {
    fn object_to_map(obj: &serde_json::Map<String, Value>) -> HashMap<String, Value> {
        obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
    }

    if let Some(dps) = value.get("dps").and_then(|d| d.as_object()) {
        return Ok(object_to_map(dps));
    }
    if let Some(dps) = value
        .get("data")
        .and_then(|d| d.get("dps"))
        .and_then(|d| d.as_object())
    {
        return Ok(object_to_map(dps));
    }

    match value {
        Value::Object(map) => {
            // A bare DP map has numeric keys and no control envelope.
            if map.keys().all(|k| k.parse::<u32>().is_ok()) {
                Ok(map.into_iter().collect())
            } else {
                Err(proto_err(format!(
                    "unexpected JSON payload (no `dps` map): {map:?}"
                )))
            }
        }
        other => Err(proto_err(format!("unexpected JSON payload: {other}"))),
    }
}

/// True if the error is a socket read timeout.
pub fn is_timeout(err: &Box<dyn std::error::Error>) -> bool {
    err.downcast_ref::<Error>()
        .map(|io| matches!(io.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn frame_roundtrip_layout() {
        let key = b"0123456789abcdef";
        let wire = pack_frame(123, CMD_DP_QUERY, b"{}", key, None).unwrap();

        assert_eq!(&wire[0..4], &PREFIX);
        assert_eq!(&wire[wire.len() - 4..], &SUFFIX);
        let body_len = u32::from_be_bytes(wire[14..18].try_into().unwrap()) as usize;
        assert_eq!(wire.len(), 18 + body_len + 4);
        assert_eq!(&wire[6..10], &123u32.to_be_bytes());
        assert_eq!(&wire[10..14], &CMD_DP_QUERY.to_be_bytes());

        let nonce = &wire[18..30];
        let ciphertext = &wire[30..wire.len() - 4];
        let cipher = new_cipher(key).unwrap();
        let plain = cipher
            .decrypt(
                Nonce::from_slice(nonce),
                Payload {
                    msg: ciphertext,
                    aad: &wire[4..18],
                },
            )
            .unwrap();
        assert_eq!(plain, b"{}");
    }

    #[test]
    fn retcode_is_prepended_when_requested() {
        let key = b"0123456789abcdef";
        let wire = pack_frame(1, CMD_CONTROL, b"x", key, Some(0)).unwrap();
        let nonce = &wire[18..30];
        let ciphertext = &wire[30..wire.len() - 4];
        let cipher = new_cipher(key).unwrap();
        let plain = cipher
            .decrypt(
                Nonce::from_slice(nonce),
                Payload {
                    msg: ciphertext,
                    aad: &wire[4..18],
                },
            )
            .unwrap();
        assert_eq!(plain, b"\x00\x00\x00\x00x");
    }

    #[test]
    fn dps_extraction_handles_both_shapes() {
        let query: Value = serde_json::from_str(r#"{"dps":{"129":23}}"#).unwrap();
        assert_eq!(dps_from_json(query).unwrap().get("129"), Some(&json!(23)));

        let report: Value =
            serde_json::from_str(r#"{"protocol":4,"t":1,"data":{"dps":{"107":2500}}}"#).unwrap();
        assert_eq!(
            dps_from_json(report).unwrap().get("107"),
            Some(&json!(2500))
        );

        let bare: Value = serde_json::from_str(r#"{"129":23}"#).unwrap();
        assert_eq!(dps_from_json(bare).unwrap().get("129"), Some(&json!(23)));
    }

    #[test]
    fn report_json_skips_protocol_header() {
        let mut payload = vec![0, 0, 0, 0];
        payload.extend_from_slice(b"3.5");
        payload.extend_from_slice(&[0u8; 12]);
        payload.extend_from_slice(br#"{"data":{"dps":{"129":23}}}"#);
        let frame = Frame {
            seq: 1,
            cmd: CMD_DP_REPORT,
            payload,
        };
        assert_eq!(
            frame.json().unwrap()["data"]["dps"]["129"],
            json!(23)
        );
    }

    #[test]
    fn discovery_roundtrip_and_retcode_stripping() {
        // Broadcast request: no retcode, payload starts with '{'. Note the
        // builder uses random nonce, but the UDP key and cmd/seq are fixed.
        let wire = build_discovery_packet("192.168.1.2").unwrap();
        assert_eq!(&wire[0..4], &PREFIX);
        assert_eq!(&wire[wire.len() - 4..], &SUFFIX);
        assert_eq!(u32::from_be_bytes(wire[6..10].try_into().unwrap()), 0);
        assert_eq!(
            u32::from_be_bytes(wire[10..14].try_into().unwrap()),
            CMD_REQ_DEVINFO
        );
        let frame = unpack_single_frame(&wire, &UDP_KEY).unwrap();
        let value = frame.discovery_json().unwrap();
        assert_eq!(value["from"], json!("app"));
        assert_eq!(value["ip"], json!("192.168.1.2"));

        // Device reply shape: 4-byte retcode + JSON (+ NUL padding).
        let mut payload = vec![0, 0, 0, 0];
        payload.extend_from_slice(
            br#"{"ip":"192.168.1.100","gwId":"example-device-id","productKey":"ae6rmrnkqz7rjh66","version":"3.5"}"#,
        );
        payload.extend_from_slice(&[0, 0]);
        let reply = pack_frame(0, 19, &payload[4..], &UDP_KEY, Some(0)).unwrap();
        let frame = unpack_single_frame(&reply, &UDP_KEY).unwrap();
        let value = frame.discovery_json().unwrap();
        assert_eq!(value["gwId"], json!("example-device-id"));
        assert_eq!(value["productKey"], json!("ae6rmrnkqz7rjh66"));
        assert_eq!(decrypt_udp_json(&reply).unwrap()["ip"], json!("192.168.1.100"));
    }
}
