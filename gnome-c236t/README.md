# C236T Indicator (GNOME Shell extension)

Top-bar temperature + output for the INKBIRD C236T, fed by the Rust CLI.

* Polls `/usr/bin/c236t-ctl status --json` (config stays in
  `~/.config/c236t/c236t.toml` — the extension never sees the key)
* Label: `34.9°C COOL_ON` (toggle "Show output" in prefs)
* Menu: temps, output, heat/cool bands, Refresh, Temp-control + Buzzer toggles

## Install

```bash
# 1. Install the CLI (built in the repo root, then copy/sudo install)
cargo build --release
sudo install -m 0755 target/release/c236t-ctl /usr/bin/c236t-ctl

# 2. Configure once (writes ~/.config/c236t/c236t.toml)
c236t-ctl setup

# 3. Install the extension (user install, no sudo needed)
mkdir -p ~/.local/share/gnome-shell/extensions/c236t@local
cp -r gnome-c236t/* ~/.local/share/gnome-shell/extensions/c236t@local/
# Compile settings schema INSIDE the extension dir
glib-compile-schemas ~/.local/share/gnome-shell/extensions/c236t@local/schemas/

# 4. Wayland: log out/in (no shell restart). X11: alt+f2 r. Then enable
gnome-extensions enable c236t@local
# System install: cp -r gnome-c236t /usr/share/gnome-shell/extensions/c236t@local/
#               sudo glib-compile-schemas /usr/share/gnome-shell/extensions/c236t@local/schemas/
```

Settings (Preferences): interval `10..300` s (default 30), binary path
(default `/usr/bin/c236t-ctl`), show output.

## Troubleshooting

* `C236T n/a / err` → run `c236t-ctl status` in a terminal; check the
  binary path, `~/.config/c236t/c236t.toml`, and the local session
  (force-stop the INKBIRD app).
* `watch --json` streams the same shape as `status --json` for custom
  dashboards.
