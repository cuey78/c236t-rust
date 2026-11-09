import Adw from 'gi://Adw';
import Gio from 'gi://Gio';
import Gtk from 'gi://Gtk';
import { ExtensionPreferences } from 'resource:///org/gnome/Shell/Extensions/js/extensions/prefs.js';

function spawnSet(binary, assignments, onDone) {
  const argv = [binary, 'set', ...assignments];
  const launcher = new Gio.SubprocessLauncher();
  launcher.set_flags(Gio.SubprocessFlags.STDOUT_PIPE | Gio.SubprocessFlags.STDERR_PIPE);
  let proc = null;
  try {
    proc = launcher.spawnv(argv);
  } catch (e) {
    log(`C236T prefs spawn failed: ${e.message}`);
    if (onDone) onDone(false, e.message);
    return;
  }
  proc.communicate_async(null, null, (p, res) => {
    const [ok, stdout, stderr] = p.communicate_finish(res);
    if (!ok) {
      if (onDone) onDone(false, 'communicate failed');
      return;
    }
    const out = new TextDecoder().decode(stdout ? stdout.get_data() : new Uint8Array());
    const err = new TextDecoder().decode(stderr ? stderr.get_data() : new Uint8Array());
    if (p.get_exit_status() !== 0) {
      if (onDone) onDone(false, err.trim() || `exit ${p.get_exit_status()}`);
      return;
    }
    if (onDone) onDone(true, out);
  });
}

function tempRow(group, title, dpName, settings) {
  const binary = () => settings.get_string('binary').trim() || '/usr/bin/c236t-ctl';
  const row = new Adw.SpinRow({
    title,
    adjustment: new Gtk.Adjustment({ lower: -40, upper: 100, step_increment: 0.5, page_increment: 5, value: 25 }),
    map: value => `${value.toFixed(1)}°`,
    digits: 1,
  });
  row.connect('notify::value', () => {
    const v = row.get_value();
    // Clamp to tenths as the CLI expects degrees.
    const degrees = Math.round(v * 10) / 10;
    spawnSet(binary(), [`${dpName}=${degrees}`], (ok, msg) => {
      log(`C236T prefs set ${dpName}=${degrees}: ${ok ? 'ok' : msg}`);
    });
  });
  group.add(row);
  return row;
}

export default class C236TPrefs extends ExtensionPreferences {
  fillPreferencesWindow(window) {
    const settings = this.getSettings();
    const page = new Adw.PreferencesPage({
      title: 'C236T Indicator',
      icon_name: 'temperature',
    });
    window.add(page);

    const group = new Adw.PreferencesGroup({
      title: 'Polling',
      description: 'How often to run: /usr/bin/c236t-ctl status --json',
    });
    page.add(group);

    const intervalRow = new Adw.SpinRow({
      title: 'Interval (seconds)',
      adjustment: new Gtk.Adjustment({ lower: 10, upper: 300, step_increment: 5, value: settings.get_int('interval') }),
    });
    intervalRow.connect('notify::value', () => {
      settings.set_int('interval', Math.round(intervalRow.get_value()));
    });
    group.add(intervalRow);

    const binaryRow = new Adw.EntryRow({ title: 'Binary path' });
    binaryRow.set_text(settings.get_string('binary'));
    binaryRow.connect('notify::text', () => {
      const v = binaryRow.get_text().trim();
      if (v) settings.set_string('binary', v);
    });
    group.add(binaryRow);

    const outputRow = new Adw.SwitchRow({
      title: 'Show output state',
      active: settings.get_boolean('show-output'),
    });
    outputRow.connect('notify::active', () => {
      settings.set_boolean('show-output', outputRow.get_active());
    });
    group.add(outputRow);

    const control = new Adw.PreferencesGroup({
      title: 'Temperature control',
      description: 'Changing a value writes it immediately via c236t-ctl set (degrees).',
    });
    page.add(control);
    tempRow(control, 'Heating start (DP 107)', 'temp_heatStart', settings);
    tempRow(control, 'Heating stop (DP 108)', 'temp_heatStop', settings);
    tempRow(control, 'Cooling start (DP 134)', 'temp_coolStart', settings);
    tempRow(control, 'Cooling stop (DP 135)', 'temp_coolStop', settings);

    const alarms = new Adw.PreferencesGroup({
      title: 'Alarms',
    });
    page.add(alarms);
    tempRow(alarms, 'High alarm (DP 109)', 'ah_alarm', settings);
    tempRow(alarms, 'Low alarm (DP 110)', 'al_alarm', settings);

    const ttlRow = new Adw.SpinRow({
      title: 'Cooling delay (DP 111, seconds)',
      adjustment: new Gtk.Adjustment({ lower: 0, upper: 3600, step_increment: 10, value: 0 }),
    });
    ttlRow.connect('notify::value', () => {
      const binary = settings.get_string('binary').trim() || '/usr/bin/c236t-ctl';
      const v = Math.round(ttlRow.get_value());
      spawnSet(binary, [`cool_cd=${v}`], (ok, msg) => {
        log(`C236T prefs set cool_cd=${v}: ${ok ? 'ok' : msg}`);
      });
    });
    alarms.add(ttlRow);
  }
}
