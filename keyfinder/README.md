# C236T Key Finder (Android helper)

One-time helper to fetch **your own** device's 16-byte `localKey` so you
can use the Linux `c236t-ctl` CLI offline. It re-uses the ThingClips SDK
bundled in the official INKBIRD app — the INKBIRD REST API exposes no
device/key endpoint.

## Install

1. Install the official **INKBIRD** Android app (`com.inkbird.inkbirdapp`),
   log in, and make sure your C236T is online there.
2. Install `key-finder.apk` (allow *install unknown apps*).
3. Open **C236T Key Finder**. Enter country code (`61` for Australia),
   email, password → **Login & find keys**.
4. Copy the shown `localKey` for your device (tap **Copy key**).

> Your password is used once to fetch the key and is never stored,
> logged, or sent anywhere except the ThingClips login. Only install
> this APK from a source you trust.

## Then on Linux

```bash
c236t-ctl discover
c236t-ctl setup --ip <ip> --id <deviceId> --key <pasted>
c236t-ctl status
```

Each device has its own key — a key only works for the unit it came from.
Never share `c236t.toml` or the key.

## Building from source

The APK bundles the official app's DEX/lib code (see `stage` flow in
`build.sh`, which re-uses the extracted `base.apk` payload) plus the
generic `FinderActivity` in this folder:

```bash
./build.sh   # requires Android tools + the official base.apk payload
```

No phone was attached at build time here, so the APK is compile-verified
only (signing + no hard-coded credentials verified by dex inspection).
Reconnect your phone and install to live-test.
