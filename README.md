# C236T Native Controller (Rust)

A Linux-native Rust CLI that reads the full status of and controls the
INKBIRD C236T WiFi temperature controller **directly over the LAN**, with
no cloud account, MQTT broker, or phone app involved at runtime.

> For your **own** device only. The controller speaks Tuya local protocol
> **3.5** (`6699` / AES-GCM) on TCP `6668`. Discovery is UDP `7000`.
> 
NOTE: Upfront Disclose: This was Created using AI reverse engineering - Since no one had a working option i made one (with help) , im not a software engineer so anyone can fork this and improve it.

## Device identification

* **Model / product:** INKBIRD C236T. Its product IDs include
  `ae6rmrnkqz7rjh66` (and related OTA/InkTem variants) — the same for
  every C236T unit.
* **Device ID (`gwId`):** unique per physical unit (like a serial
  number). Not a key, not shared, not needed for crypto.
* **`localKey`:** 16-byte per-device secret for the AES handshake.
  Unique to your unit. Never broadcast.
* **`UDP key`:** `MD5(b"yGAdlopoPVldABfn")` — a single generic key
  shared by *all* Tuya devices, used only to encrypt discovery
  broadcasts. `discover` auto-fills IP / device ID / product, never the
  local key.

## Requirements

* Rust (stable) + `cargo`
* One Android phone with the official INKBIRD app (for the one-time
  Key Finder extraction below), or an existing local key

## Quick start

```bash
cd c236t-rust
cargo build --release

# 1. Find the device (UDP broadcast)
./target/release/c236t-ctl discover

# 2. Get your unit's local key with the Key Finder APK
#    (see `keyfinder/README.md`; one-time, your own account)

# 3. Configure (prompts IP / port / key, verifies, writes c236t.toml)
./target/release/c236t-ctl setup

# 4. Use it
./target/release/c236t-ctl status
./target/release/c236t-ctl get current_temp
./target/release/c236t-ctl set temp_heatStart=25 temp_heatStop=26
./target/release/c236t-ctl watch
```

## How it works — Tuya 3.5 protocol

### Frame layout

```
prefix(4)=00 00 66 99 | reserved(2)=00 00 | seq(4) | cmd(4) | length(4)
| nonce(12) | ciphertext | tag(16) | suffix(4)=00 00 99 66
```

* All integers big-endian. `length` covers nonce + ciphertext + tag.
* AES-GCM AAD is header bytes `4..18` (`reserved | seq | cmd | length`).
* Every packet uses a fresh 12-byte GCM nonce (the first 12 bytes of the
  wire body). The 16-byte tag is the GCM authentication tag.

### Handshake (encrypted with the 16-byte `localKey`)

```
C -> D  cmd 3 : client_nonce(16)                          [encrypted, localKey]
D -> C  cmd 4 : retcode(4) + device_nonce(16)
                + HMAC-SHA256(localKey, client_nonce)     [encrypted, localKey]
C -> D  cmd 5 : HMAC-SHA256(localKey, device_nonce)       [encrypted, localKey]

session_key = AES-GCM(localKey, nonce=client_nonce[0..12],
                      plaintext=device_nonce XOR client_nonce)[0..16]
```

Verification: the CLI checks the cmd-4 HMAC. A mismatch means the local
key is wrong (or the official app still holds the session — force-stop
it first).

### Data commands (session key)

| cmd | Direction | Purpose | Payload |
|-----|-----------|---------|---------|
| 3/4/5 | both | session-key negotiation | see above |
| 37 (`0x25`) | C→LAN | `REQ_DEVINFO` discovery broadcast (UDP 7000) | `{"from":"app","ip":"<local-ip>"}` |
| 8 | device → client | DP report | `retcode + "3.5" + 12B header + JSON` |
| 9 | both | heartbeat | empty; device acks |
| 13 | client → device | control | `"3.5" + 12 zero bytes + JSON` |
| 16 | client → device | DP query | `{}` |

Device-originated payloads start with a 4-byte big-endian `retcode` (must
be `0`). Report JSON shape:

```json
{"protocol":4,"t":…,"data":{"dps":{"129":…}}}
```

Query response shape: `{"dps":{…}}`. The code accepts both plus a bare
DP map (`{"129":…}`).

Control JSON shape (CLI builds the `"3.5"` envelope automatically):

```json
{"data":{"dps":{"111":0}},"protocol":5,"t":<unix>}
```

`watch` sends heartbeats every 10 s and prints cmd-8 reports with
auto-reconnect. `status` uses `query_snapshot` (query + short settle)
because firmware splits the full snapshot across the cmd-16 response and
several cmd-8 reports.

### Discovery (UDP)

* Broadcast `REQ_DEVINFO` (cmd 37, seq 0) to `255.255.255.255:7000`
  plus the subnet `.255`, encrypted with the shared UDP key, payload
  `{"from":"app","ip":"<local-ip>"}`. No retcode on the request.
* Listen on `6666`/`6667`/`7000` (broadcast socket, re-send every 2 s)
  for the device reply (cmd 19): `retcode + devinfo JSON` where devinfo
  carries `ip`, `gwId`, `productKey`, `version`, `uuid`.
* Repeats ignore own `{"from":"app"}` echoes. The local key is never in
  discovery responses.

## CLI reference

```
c236t-ctl status                 Read and print every data point
c236t-ctl get <dp>               Read one data point (<id> or <name>)
c236t-ctl set <dp>=<value> ...   Write one or more data points
c236t-ctl watch                  Stream live reports (auto-reconnect)
c236t-ctl raw '<json>'           Raw dps object or control JSON
c236t-ctl list                   List the DP table
c236t-ctl discover [--timeout <secs>]  Find Tuya devices (UDP 7000)
c236t-ctl setup [--ip <ip>] [--port <port>] [--key <localKey>]
                                 Interactive + verified c236t.toml writer
                                 (--id accepted, never prompted)
```

Add `--json` for machine-readable output (extensions, scripts, GNOME):

* `status --json` → `{device_id,ip,port,count,dps}` (raw wire values)
* `get <dp> --json` → `{id,name,value,display,description}`
* `list --json` → `[{id,name,kind,description}, …]`
* `discover --json` → `[{ip,id,product_key,version,uuid,is_c236t}, …]`
* `set <dp>=<v> --json` → `{applied,verified}`
* `watch --json` → JSON Lines: one `{"type":"snapshot",…}` then
  `{"type":"report",…}` per line
* `raw --json` → `{"ok":true}`; `setup` rejects `--json`

Temperatures in JSON are raw wire values (hundredths: `4028` = 40.28°).
DP IDs are strings (`"129"`), values keep their wire types.

Value handling:

* **Bool / trigger:** `on`/`off`, `true`/`false`, `1`/`0`
* **Temperature (`temp`):** degrees, e.g. `25` or `25.5`; on the wire as
  hundredths (`2500`, `2550`)
* **Int:** raw integer
* **Text:** literal string (DP 121/132/133/145 are opaque hex/binary)

## Data points

`c236t-ctl list` is the source of truth. All DPs the firmware exposes are
printed even if unknown, so new fields are never silently dropped.

| DP | Name | Type | Notes |
|----|------|------|-------|
| 101 | `all` | trigger | query / report-all |
| 102 | `guide_sw` | int | setup-wizard step |
| 103 | `workMode` | int | 0 temp, 1 time, 2 temp+time, 3 manual |
| 104 | `timeMode` | int | 0 cycle, 1 countdown on, 2 countdown off, 3 on+off |
| 105 | `tempTimeMode` | int | as 104 for temp+time |
| 106 | `unit` | bool | false = °C, true = °F |
| 107/108 | `temp_heatStart/Stop` | temp | heating hysteresis |
| 109/110 | `ah_alarm / al_alarm` | temp | alarm thresholds |
| 111 | `cool_cd` | int | cooling delay (s) |
| 112 | `ca` | int | calibration offset |
| 113/114 | `ah_sw / al_sw` | bool | alarm enables |
| 115 | `bg_light_sw` | bool | display/backlight |
| 116 | `kid_lock` | bool | child lock |
| 117 | `temp_ctrl_sw` | bool | temperature control enable |
| 118 | `manual_sw` | bool | manual output power |
| 119 | `P_ctrl_sw` | bool | program/schedule enable |
| 120 | `buz_sw` | bool | buzzer enable |
| 121 | `P_ctrl_set` | text | 8 schedule entries (10 hex chars each) |
| 129 | `current_temp` | temp | measured temperature (read-only) |
| 130 | `output_mode` | text | `HEAT_ON/HEAT_OFF/COOL_ON/COOL_OFF` (read-only) |
| 131 | `alarm_history` | text | read-only |
| 132/133 | `history_record / timeWorkInfo` | text | runtime stats (read-only) |
| 134/135 | `temp_coolStart/Stop` | temp | cooling hysteresis |
| 136–139 | `tempTime_*` | temp | temp+time heat/cool start/stop |
| 145 | `alarm` | text | current alarm (read-only) |
| 146 | `cool_delay_sw` | bool | cooling delay enable |
| 147 | `clear_dev_data` | trigger | write-only |
| 148 | `mute_now_dev_alarm` | trigger | write-only |
| 149 | `reserved_149` | text | firmware-specific |
| 150 | `ota_update` | bool | firmware OTA flag |

Live-verified on a real device: `status` returns ~30 DPs, `get`/`set`
round-trip, `raw` control, `watch` reports, and physical output changes.

## Configuration

Use `setup` (see Quick start), or copy `c236t.example.toml` to
`~/.config/c236t/c236t.toml` and edit it manually (`setup` creates the
folder `~/.config/c236t/` with `0700` if missing; `./c236t.toml` upward
search and `C236T_CONFIG_FILE` are fallbacks):

```toml
# c236t.toml
local_key = "your-16-byte-local-key"
ip = "192.168.1.100"
port = 6668
device_id = "your-device-id"   # optional, display only
```

| Setting | Required | Default | Env override |
|---------|----------|---------|--------------|
| `local_key` | yes | – | `THING_LOCAL_KEY` |
| `ip` | yes | – | `THING_LOCAL_IP` |
| `port` | no | `6668` | `THING_LOCAL_PORT` |
| `device_id` | no | – | `THING_DEV_ID` |

Precedence: shell env > `~/.config/c236t/c236t.toml` (XDG) >
`./c236t.toml` > legacy `.env`.

TOML was chosen over `.env`/plain text and JSON: it is unambiguous,
supports comments, and the serializer handles quoting/escaping for any
16-byte key. JSON has no comments; ad-hoc `key=value` reintroduces the
same `$`/`"`/`\` escaping bugs as `.env`.

`~/.config/c236t/c236t.toml` is written `0600` (dir `0700`). `setup`
only saves after a successful handshake + query; a wrong key never
overwrites an existing file.

> The local key is per-device and is **not** the account password, and
> is not revealed by discovery.

## Getting a local key

The INKBIRD REST API (`POST smartAgent/user/login`) works but exposes
**no** device list or local-key endpoint (`deviceList` → 404). The
device record lives in the Tuya/ThingClips cloud (`thing.m.device.get`),
protected by the app's native JNI signer.

Options (best first):

1. **Key Finder APK** (this repo, `keyfinder/`) — one-time on-device
   extraction with your own account. See below.
2. Port Tuya's native signer to Rust — large, brittle, breaks on app
   updates.
3. Capture / Frida on the official app — fragile, for experts.
4. Plain INKBIRD login or `discover` alone — **will not** yield the key.

### Key Finder APK

`key-finder.apk` (ship via GitHub Releases, not the repo) plus source
in `keyfinder/`:

* Bundles the ThingClips SDK/DEX from the official `base.apk` plus a
  generic `FinderActivity` (no hard-coded email/password/device).
* UI: country code, email, password → login → `m.life.home.space.list` →
  `m.life.app.home.data.list` (with `setGid`) → per-device
  `thing.m.device.get` → shows `localKey` with copy-to-clipboard.
* Password is in-memory only; it is never logged/persisted. Install
  only from a trusted source. Each unit's key only works for that unit.

## Build & test

```bash
cargo build --release   # -> target/release/c236t-ctl
cargo test --release    # 18 unit tests (frames, handshake, DP parsing,
                        # discovery round-trip, TOML round-trip)
```

## Troubleshooting

* **Handshake HMAC mismatch / closed connection:** wrong local key, or
  the official app is holding the session:
  `adb shell am force-stop com.inkbird.inkbirdapp`. Only one local
  client at a time.
* **No devices found:** same WiFi/LAN, allow UDP `7000` in the
  firewall, device online, try `discover --timeout 15`.
* **`no device IP found`:** run `setup` (or set `ip` in `c236t.toml`).
* **Key file won't load:** delete/re-run `setup`; verify TOML syntax
  (`invalid TOML config…` points at the line).

## Privacy / what is (not) committed

`.gitignore` excludes secrets and binaries:

```
target/  .env  c236t.toml  config.toml  .c236t_token  *.keystore
*.apk  *.aab  base.apk  *.log  *.pcap
```

Examples use `192.168.1.100` / `your-device-id` / `your-16-byte-local-key`.
Model/product IDs are generic (needed for C236T detection). The Key
Finder's `APP_ID`/`APP_SECRET` are the public values embedded in the
official app binary, required for SDK init.

## Project layout

```
c236t-rust/
  src/
    main.rs       CLI: status/get/set/watch/raw/list/discover/setup
    protocol.rs   6699 frames, handshake, session key, DP query/control
    discovery.rs  UDP REQ_DEVINFO scanner
    config.rs     c236t.toml load/save (TOML)
    dp.rs         DP table (101–150)
    auth.rs, mqtt.rs  legacy cloud modules (reference only)
  c236t.example.toml  .env.example
  keyfinder/      Android helper (source + build script)
  README.md       this file
```

## Validation & reverse-engineering sources

* Official APK (`base.apk`, not committed): `C236tConstant` (DP IDs),
  `C236tManager` (DP parsing), `libnetwork-android.so` (6699/GCM),
  `ThingApiSignManager` (native signer)
* Captured LAN session (PCAPdroid) — handshake/frame validation
* Live device — `status`/`get`/`set`/`watch`/`raw`/`discover`/`setup`
* TinyTuya source — discovery constants (`REQ_DEVINFO=0x25`, UDP
  6666/6667/7000, `udpkey = MD5("yGAdlopoPVldABfn")`)

## Licence

MIT. Not affiliated with INKBIRD or Tuya. Use only on devices you own.
