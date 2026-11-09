import { Extension } from 'resource:///org/gnome/shell/extensions/extension.js';
import St from 'gi://St';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Clutter from 'gi://Clutter';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as PanelMenu from 'resource:///org/gnome/shell/ui/panelMenu.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';

const BIN_FALLBACK = '/usr/bin/c236t-ctl';

export default class C236TExtension extends Extension {
  enable() {
    this._settings = this.getSettings();
    // Direct PanelMenu.Button (no subclass — avoids GType registration).
    this._button = new PanelMenu.Button(0.0, 'C236T Indicator');
    this._label = new St.Label({
      text: 'C236T …',
      style_class: 'c236t-label',
      y_align: Clutter.ActorAlign.CENTER,
    });
    const box = new St.BoxLayout({ style_class: 'panel-status-menu-box' });
    box.add_child(this._label);
    this._button.add_child(box);

    const self = this;
    this._button.menu.addMenuItem(new PopupMenu.PopupSeparatorMenuItem('C236T — INKBIRD'));
    this._tempItem = new PopupMenu.PopupMenuItem('Temperature: …');
    this._outputItem = new PopupMenu.PopupMenuItem('Output: …');
    this._extraItem = new PopupMenu.PopupMenuItem('…');
    this._button.menu.addMenuItem(this._tempItem);
    this._button.menu.addMenuItem(this._outputItem);
    this._button.menu.addMenuItem(this._extraItem);

    const refresh = new PopupMenu.PopupMenuItem('Refresh now');
    refresh.connect('activate', () => self._update());
    this._button.menu.addMenuItem(refresh);

    const mkToggle = (label, setter) => {
      const sw = new PopupMenu.PopupSwitchMenuItem(label, false);
      sw.connect('toggled', (item, on) => {
        self._runBin([setter(on)], () => self._update(), () => self._update());
      });
      sw.setSensitive(false);
      self._button.menu.addMenuItem(sw);
      return sw;
    };
    this._ctrlSw = mkToggle('Temp control', on => `set temp_ctrl_sw=${on ? 'on' : 'off'}`);
    this._buzSw = mkToggle('Buzzer', on => `set buz_sw=${on ? 'on' : 'off'}`);
    this._switches = [this._ctrlSw, this._buzSw];

    Main.panel.addToStatusArea('c236t-indicator', this._button, 1, 'right');
    this._update();
    this._timer = null;
    this._startTimer();
    this._intervalChanged = this._settings.connect('changed::interval', () => {
      this._startTimer();
    });
  }

  disable() {
    if (this._timer !== null) {
      GLib.source_remove(this._timer);
      this._timer = null;
    }
    if (this._button) {
      this._button.destroy();
      this._button = null;
    }
    if (this._intervalChanged) {
      this._settings.disconnect(this._intervalChanged);
      this._intervalChanged = null;
    }
    this._settings = null;
  }

  _startTimer() {
    if (this._timer !== null) GLib.source_remove(this._timer);
    const secs = Math.max(10, Math.min(300, this._settings.get_int('interval')));
    this._timer = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, secs, () => {
      this._update();
      return true;
    });
  }

  _binary() {
    const b = this._settings.get_string('binary').trim();
    return b || BIN_FALLBACK;
  }

  _update() {
    this._runBin(['status', '--json'], out => {
      try {
        const data = JSON.parse(out);
        const dps = data.dps || {};
        const rawTemp = dps['129'];
        const unitF = dps['106'];
        const output = dps['130'] || '';
        const heatStart = dps['107'];
        const heatStop = dps['108'];
        const coolStart = dps['134'];
        const coolStop = dps['135'];
        if (typeof rawTemp !== 'number') throw new Error('no current_temp');
        const temp = rawTemp / 100;
        const unit = unitF ? '°F' : '°C';
        const showOutput = this._settings.get_boolean('show-output');
        this._label.text = showOutput
            ? `${temp.toFixed(1)}${unit} ${output}`
            : `${temp.toFixed(1)}${unit}`;
        this._tempItem.label.text = `Temperature: ${temp.toFixed(2)}${unit}`;
        this._outputItem.label.text = `Output: ${output}`;
        const fmt = v => (typeof v === 'number' ? `${(v / 100).toFixed(1)}${unit}` : '—');
        this._extraItem.label.text =
            `Heat ${fmt(heatStart)}→${fmt(heatStop)} · Cool ${fmt(coolStart)}→${fmt(coolStop)}`;
        const ctrlOn = !!dps['117'];
        const buzOn = !!dps['120'];
        this._ctrlSw.setToggleState(ctrlOn);
        this._buzSw.setToggleState(buzOn);
        this._switches.forEach(s => s.setSensitive(true));
      } catch (e) {
        this._label.text = 'C236T n/a';
        this._tempItem.label.text = `Error: ${e.message || e}`;
        this._outputItem.label.text = 'Check config: ~/.config/c236t/c236t.toml';
        this._extraItem.label.text = 'Run: /usr/bin/c236t-ctl status';
        this._switches.forEach(s => s.setSensitive(false));
      }
    }, err => {
      this._label.text = 'C236T err';
      this._tempItem.label.text = `Error: ${err}`;
      this._outputItem.label.text = 'Check: /usr/bin/c236t-ctl status';
      this._extraItem.label.text = 'Check: ~/.config/c236t/c236t.toml';
      this._switches.forEach(s => s.setSensitive(false));
    });
  }

  _runBin(args, onOk, onErr) {
    const argv = [this._binary(), ...args];
    const launcher = new Gio.SubprocessLauncher();
    launcher.set_cwd(null);
    let proc = null;
    try {
      proc = launcher.spawnv(argv);
    } catch (e) {
      if (onErr) onErr(`spawn failed: ${e.message}`);
      return;
    }
    proc.communicate_async(null, null, (p, res) => {
      const [ok, stdout, stderr] = p.communicate_finish(res);
      if (!ok) {
        if (onErr) onErr('communicate failed');
        return;
      }
      const out = new TextDecoder().decode(stdout);
      if (p.get_exit_status() !== 0) {
        const err = new TextDecoder().decode(stderr || new Uint8Array());
        if (onErr) onErr(err.trim() || `exit ${p.get_exit_status()}`);
        return;
      }
      if (onOk) onOk(out);
    });
  }
}
