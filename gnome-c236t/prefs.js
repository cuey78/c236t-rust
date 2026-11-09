import Adw from 'gi://Adw';
import Gio from 'gi://Gio';
import Gtk from 'gi://Gtk';
import { ExtensionPreferences } from 'resource:///org/gnome/Shell/Extensions/prefs.js';

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
  }
}
