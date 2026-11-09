import Adw from 'gi://Adw';
import Gio from 'gi://Gio';
import Gtk from 'gi://Gtk';
import { ExtensionPreferences } from 'resource:///org/gnome/Shell/Extensions/js/extensions/prefs.js';

function spawnBin(binary, args, onDone) {
  const argv = [binary, ...args];
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

export default class C236TPrefs extends ExtensionPreferences {
  fillPreferencesWindow(window) {
    const settings = this.getSettings();
    const binary = () => settings.get_string('binary').trim() || '/usr/bin/c236t-ctl';

    // Guard so programmatic fills don't trigger writes.
    let filling = false;

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

    // dp name -> { row, id, divide } for device fill + write-back.
    const controls = [];

    const makeTempRow = (parent, title, dpName, dpId) => {
      const row = new Adw.SpinRow({
        title,
        adjustment: new Gtk.Adjustment({ lower: -40, upper: 100, step_increment: 0.5, page_increment: 5, value: 25 }),
        digits: 1,
      });
      row.connect('notify::value', () => {
        if (filling) return;
        const degrees = Math.round(row.get_value() * 10) / 10;
        spawnBin(binary(), ['set', `${dpName}=${degrees}`], (ok, msg) => {
          log(`C236T prefs set ${dpName}=${degrees}: ${ok ? 'ok' : msg}`);
        });
      });
      parent.add(row);
      controls.push({ row, dpId, divide: 100 });
      return row;
    };

    const control = new Adw.PreferencesGroup({
      title: 'Temperature control',
      description: 'Changing a value writes it immediately via c236t-ctl set (degrees). Values load from the device.',
    });
    page.add(control);
    makeTempRow(control, 'Heating start (DP 107)', 'temp_heatStart', '107');
    makeTempRow(control, 'Heating stop (DP 108)', 'temp_heatStop', '108');
    makeTempRow(control, 'Cooling start (DP 134)', 'temp_coolStart', '134');
    makeTempRow(control, 'Cooling stop (DP 135)', 'temp_coolStop', '135');

    const alarms = new Adw.PreferencesGroup({
      title: 'Alarms',
    });
    page.add(alarms);
    makeTempRow(alarms, 'High alarm (DP 109)', 'ah_alarm', '109');
    makeTempRow(alarms, 'Low alarm (DP 110)', 'al_alarm', '110');

    const ttlRow = new Adw.SpinRow({
      title: 'Cooling delay (DP 111, seconds)',
      adjustment: new Gtk.Adjustment({ lower: 0, upper: 3600, step_increment: 10, value: 0 }),
    });
    ttlRow.connect('notify::value', () => {
      if (filling) return;
      const v = Math.round(ttlRow.get_value());
      spawnBin(binary(), ['set', `cool_cd=${v}`], (ok, msg) => {
        log(`C236T prefs set cool_cd=${v}: ${ok ? 'ok' : msg}`);
      });
    });
    alarms.add(ttlRow);
    controls.push({ row: ttlRow, dpId: '111', divide: 1 });

    // Load current device values so reopening prefs shows real numbers.
    spawnBin(binary(), ['status', '--json'], (ok, out) => {
      if (!ok) {
        log(`C236T prefs status failed: ${out}`);
        return;
      }
      try {
        const dps = JSON.parse(out).dps || {};
        filling = true;
        for (const c of controls) {
          const raw = dps[c.dpId];
          if (typeof raw === 'number') c.row.set_value(raw / c.divide);
        }
        filling = false;
      } catch (e) {
        log(`C236T prefs parse failed: ${e.message}`);
      }
    });
  }
}
