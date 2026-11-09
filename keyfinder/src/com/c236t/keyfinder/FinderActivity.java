package com.c236t.keyfinder;

import android.app.Activity;
import android.content.ClipData;
import android.content.ClipboardManager;
import android.content.Context;
import android.content.ContextWrapper;
import android.content.Intent;
import android.os.Bundle;
import android.os.Looper;
import android.text.InputType;
import android.text.method.PasswordTransformationMethod;
import android.util.Log;
import android.view.View;
import android.view.ViewGroup;
import android.widget.Button;
import android.widget.EditText;
import android.widget.LinearLayout;
import android.widget.ScrollView;
import android.widget.TextView;
import android.widget.Toast;

import java.lang.reflect.Constructor;
import java.lang.reflect.Field;
import java.lang.reflect.InvocationHandler;
import java.lang.reflect.Method;
import java.lang.reflect.Proxy;
import java.util.ArrayList;
import java.util.List;

import org.json.JSONArray;
import org.json.JSONObject;

/**
 * Generic C236T key finder.
 * Logs in with the user's own INKBIRD email/password, lists devices via
 * the ThingClips SDK (same code as the official app), then fetches each
 * device record to show its 16-byte localKey.
 * Password is kept in memory only and never logged or persisted.
 */
public class FinderActivity extends Activity {
    static final String TAG = "KEYFINDER";
    static final String APP_ID = "nyfyxycreykp7jp4e593";
    static final String APP_SECRET = "vupma4jwux4e7m3jry7sspcqhvw3gwet";
    // Dummy PhoneUtil id, same idea as the probe (avoids corrupt prefs).
    static final String DUMMY_DEVICE_ID = "6385dcc6a6821b45";

    private Context officialCtx;
    private boolean sdkReady = false;

    private EditText phoneCodeEdit;
    private EditText emailEdit;
    private EditText passwordEdit;
    private Button loginButton;
    private TextView statusView;
    private LinearLayout resultsLayout;

    @Override protected void onCreate(Bundle bundle) {
        super.onCreate(bundle);
        buildUi();
        appendStatus("Starting SDK…");
        if (initSdk()) {
            appendStatus("SDK ready. Enter your INKBIRD email + password, then tap Login & find keys.");
        } else {
            appendStatus("SDK init failed — see logcat tag KEYFINDER.");
        }

        // Optional auto-run for adb testing: extras email/password/phoneCode.
        Intent intent = getIntent();
        if (intent != null) {
            String email = intent.getStringExtra("email");
            String password = intent.getStringExtra("password");
            String phoneCode = intent.getStringExtra("phoneCode");
            if (email != null && password != null) {
                if (phoneCode != null) phoneCodeEdit.setText(phoneCode);
                emailEdit.setText(email);
                passwordEdit.setText(password);
                loginButton.performClick();
            }
        }
    }

    private void buildUi() {
        ScrollView scroll = new ScrollView(this);
        LinearLayout root = new LinearLayout(this);
        root.setOrientation(LinearLayout.VERTICAL);
        int pad = dp(16);
        root.setPadding(pad, pad, pad, pad);
        LinearLayout.LayoutParams lp = new LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT);
        root.setLayoutParams(lp);
        scroll.addView(root);

        TextView title = new TextView(this);
        title.setText("C236T Key Finder");
        title.setTextSize(22);
        root.addView(title);

        TextView info = new TextView(this);
        info.setText("Install the official INKBIRD app first and make sure your C236T is online there.\n"
                + "Then enter the same email/password here (country code +61 for Australia).\n"
                + "Nothing is stored — your password is used once to fetch the key for your own device.\n"
                + "After you get the key, run on your PC:\n  c236t-ctl discover\n  c236t-ctl setup --key <key>");
        info.setTextSize(13);
        root.addView(info);

        phoneCodeEdit = new EditText(this);
        phoneCodeEdit.setHint("Country code (e.g. 61)");
        phoneCodeEdit.setText("61");
        phoneCodeEdit.setInputType(InputType.TYPE_CLASS_NUMBER);
        root.addView(phoneCodeEdit);

        emailEdit = new EditText(this);
        emailEdit.setHint("Email");
        emailEdit.setInputType(InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_EMAIL_ADDRESS);
        root.addView(emailEdit);

        passwordEdit = new EditText(this);
        passwordEdit.setHint("Password");
        passwordEdit.setInputType(InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_PASSWORD);
        passwordEdit.setTransformationMethod(PasswordTransformationMethod.getInstance());
        root.addView(passwordEdit);

        loginButton = new Button(this);
        loginButton.setText("Login & find keys");
        loginButton.setOnClickListener(new View.OnClickListener() {
            @Override public void onClick(View v) {
                String phoneCode = phoneCodeEdit.getText().toString().trim();
                String email = emailEdit.getText().toString().trim();
                String password = passwordEdit.getText().toString();
                if (phoneCode.isEmpty()) phoneCode = "61";
                if (email.isEmpty() || password.isEmpty()) {
                    appendStatus("Email and password are required.");
                    return;
                }
                if (!sdkReady && !initSdk()) return;
                resultsLayout.removeAllViews();
                appendStatus("Logging in…");
                loginButton.setEnabled(false);
                doLogin(phoneCode, email, password);
            }
        });
        root.addView(loginButton);

        statusView = new TextView(this);
        statusView.setTextSize(13);
        statusView.setPadding(0, dp(8), 0, dp(8));
        root.addView(statusView);

        resultsLayout = new LinearLayout(this);
        resultsLayout.setOrientation(LinearLayout.VERTICAL);
        root.addView(resultsLayout);

        setContentView(scroll);
    }

    private int dp(int value) {
        return (int) (value * getResources().getDisplayMetrics().density);
    }

    private void appendStatus(final String s) {
        runOnUiThread(new Runnable() {
            @Override public void run() {
                if (statusView != null) statusView.append(s + "\n");
                Log.i(TAG, s);
            }
        });
    }

    private void setStatus(final String s) {
        runOnUiThread(new Runnable() {
            @Override public void run() {
                if (statusView != null) statusView.setText(s + "\n");
                Log.i(TAG, s);
            }
        });
    }

    // ---- SDK init (same sequence as the probe) ----

    private boolean initSdk() {
        if (sdkReady) return true;
        try {
            Log.i(TAG, "init begin");
            Context official = new ContextWrapper(this) {
                @Override public String getPackageName() {
                    return "com.inkbird.inkbirdapp";
                }
            };
            officialCtx = official;

            Class<?> net = Class.forName("com.thingclips.smart.android.network.ThingSmartNetWork");
            setStatic(net, "mAppSecret", APP_SECRET);
            setStatic(net, "mAppId", APP_ID);
            setStatic(net, "mD", Boolean.FALSE);
            setStatic(net, "mAppContext", official);

            Class<?> phoneUtil = Class.forName("com.thingclips.smart.android.common.utils.PhoneUtil");
            Field devIdField = phoneUtil.getDeclaredField("mDeviceId");
            devIdField.setAccessible(true);
            devIdField.set(null, DUMMY_DEVICE_ID);

            Class<?> sdk = Class.forName("com.thingclips.smart.android.base.ThingSmartSdk");
            Method sdkInit = sdk.getMethod("init", android.app.Application.class);
            sdkInit.setAccessible(true);
            sdkInit.invoke(null, (android.app.Application) getApplicationContext());

            Class<?> sec = Class.forName("com.thingclips.sdk.network.ThingNetworkSecurity");
            Method initJni = sec.getMethod("initJNI", Context.class);
            initJni.setAccessible(true);
            initJni.invoke(null, official);

            Class<?> secStore = Class.forName("com.thingclips.sdk.security.SecuredPreferenceStore");
            Method storeInit = secStore.getMethod("init", Context.class,
                    Class.forName("com.thingclips.sdk.network.pbddddb"));
            storeInit.setAccessible(true);
            storeInit.invoke(null, official, null);

            Method initialize = null;
            for (Method m : net.getMethods()) {
                if (m.getName().equals("initialize") && m.getParameterCount() == 7) {
                    initialize = m;
                }
            }
            if (initialize == null) throw new IllegalStateException("no initialize(7) found");
            initialize.setAccessible(true);
            Class<?> apiProvider = Class.forName("com.thingclips.smart.android.base.provider.ApiUrlProvider");
            Constructor<?> ctor = apiProvider.getConstructor(Context.class);
            ctor.setAccessible(true);
            Object provider = ctor.newInstance(official);
            initialize.invoke(null, official, APP_ID, APP_SECRET, "android", "2.1.18", "2.1.18", provider);

            sdkReady = true;
            Log.i(TAG, "init ok");
            return true;
        } catch (Throwable e) {
            Log.e(TAG, "init failed", e);
            return false;
        }
    }

    private void setStatic(Class<?> c, String name, Object value) throws Exception {
        Field f = c.getField(name);
        f.setAccessible(true);
        f.set(null, value);
    }

    private Object getField(Object o, String name) throws Exception {
        Class<?> c = o.getClass();
        while (c != null && c != Object.class) {
            try {
                Field f = c.getDeclaredField(name);
                f.setAccessible(true);
                return f.get(o);
            } catch (NoSuchFieldException e) {
                c = c.getSuperclass();
            }
        }
        return null;
    }

    // ---- Login ----

    private void doLogin(final String phoneCode, final String email, final String password) {
        try {
            Class<?> safe = Class.forName("com.thingclips.smart.android.common.utils.SafeHandler");
            Constructor<?> ctor2 = safe.getConstructor(Activity.class, Looper.class);
            ctor2.setAccessible(true);
            Object handler = ctor2.newInstance(this, Looper.getMainLooper());

            Class<?> user = Class.forName("com.thingclips.sdk.user.dqdbbqp");
            Constructor<?> uct = user.getConstructor(Context.class, safe);
            uct.setAccessible(true);
            Object userApi = uct.newInstance(officialCtx, handler);

            Class<?> callbackType = Class.forName("com.thingclips.smart.android.user.api.IUidLoginCallback");
            InvocationHandler ih = new InvocationHandler() {
                @Override public Object invoke(Object proxy, Method method, Object[] args) throws Throwable {
                    String name = method.getName();
                    Log.i(TAG, "login cb " + name);
                    if ("onSuccess".equals(name) && args != null && args.length > 0) {
                        String sid = null;
                        long gid = 0;
                        try { sid = (String) getField(args[0], "sid"); } catch (Throwable ignore) {}
                        try {
                            if (args.length > 1 && args[1] instanceof Number) gid = ((Number) args[1]).longValue();
                            if (gid == 0) {
                                Object g = getField(args[0], "gid");
                                if (g instanceof Number) gid = ((Number) g).longValue();
                            }
                        } catch (Throwable ignore) {}
                        final String sidF = sid;
                        final long gidF = gid;
                        Log.i(TAG, "login ok gid=" + gidF);
                        appendStatus("Logged in. Fetching homes…");
                        passwordEdit.setText("");
                        fetchHomes(sidF, gidF, handler0(handler));
                    } else {
                        String detail = "";
                        if (args != null) {
                            for (Object a : args) if (a != null) detail += a + " ";
                        }
                        appendStatus("Login failed (" + name + "): " + detail);
                        Log.i(TAG, "login failed " + name + " " + detail);
                        enableLogin();
                    }
                    return null;
                }
            };
            Object callback = Proxy.newProxyInstance(callbackType.getClassLoader(),
                    new Class<?>[] {callbackType}, ih);

            Method login = null;
            for (Method m : user.getMethods()) {
                if (m.getName().equals("bdpdqbp") && m.getParameterCount() == 5
                        && m.getParameterTypes()[0] == String.class
                        && m.getParameterTypes()[1] == String.class
                        && m.getParameterTypes()[2] == String.class
                        && m.getParameterTypes()[3] == boolean.class
                        && m.getParameterTypes()[4] == callbackType) {
                    login = m;
                }
            }
            if (login == null) throw new IllegalStateException("no login overload");
            login.setAccessible(true);
            // Do not log password.
            login.invoke(userApi, phoneCode, email, password, true, callback);
        } catch (Throwable e) {
            Log.e(TAG, "doLogin failed", e);
            appendStatus("Login error: " + e.getMessage());
            enableLogin();
        }
    }

    // Small helper to avoid unused warning if handler not needed elsewhere.
    private Object handler0(Object h) { return h; }

    private void enableLogin() {
        runOnUiThread(new Runnable() {
            @Override public void run() { if (loginButton != null) loginButton.setEnabled(true); }
        });
    }

    // ---- Home / device listing ----

    private void fetchHomes(final String sid, final long gid, final Object handler) {
        // m.life.home.space.list — best effort to learn homes; fall back to gid.
        callRaw("HOMES", "m.life.home.space.list", "1.0", null, sid, null,
            new InitCallback() {
                @Override public void onResult(String origin) {
                    List<String> homeIds = new ArrayList<String>();
                    parseHomeIds(origin, homeIds);
                    // Always also try gid itself (probe showed gid works).
                    if (!homeIds.contains(String.valueOf(gid))) homeIds.add(String.valueOf(gid));
                    if (homeIds.isEmpty()) {
                        appendStatus("No homes found.");
                        enableLogin();
                        return;
                    }
                    appendStatus("Homes: " + homeIds);
                    // Fetch devices for each home (sequential via index).
                    fetchDevicesForHomes(sid, gid, homeIds, 0);
                }
            });
    }

    private void fetchDevicesForHomes(final String sid, final long gid,
            final List<String> homeIds, final int index) {
        if (index >= homeIds.size()) {
            appendStatus("Done. Copy your key above.");
            enableLogin();
            return;
        }
        final String homeId = homeIds.get(index);
        Object[][] post = new Object[][] {
            {"homeId", Long.valueOf(homeId)},
            {"classify", Integer.valueOf(0)},
            {"noRoomFilter", Boolean.TRUE}
        };
        callRaw("HOME_DEVICES", "m.life.app.home.data.list", "1.0", post, sid, Long.valueOf(gid),
            new InitCallback() {
                @Override public void onResult(String origin) {
                    final List<String> devIds = new ArrayList<String>();
                    final List<String> devNames = new ArrayList<String>();
                    parseDeviceIds(origin, devIds, devNames);
                    if (devIds.isEmpty()) {
                        appendStatus("No devices in home " + homeId);
                        fetchDevicesForHomes(sid, gid, homeIds, index + 1);
                        return;
                    }
                    appendStatus("Found " + devIds.size() + " device(s) in home " + homeId);
                    fetchOneDevice(devIds, devNames, 0, sid, gid, homeIds, homeIds, index);
                }
            });
    }

    private void fetchOneDevice(final List<String> devIds, final List<String> devNames,
            final int idx, final String sid, final long gid,
            final List<String> homeIds, final List<String> allHomes, final int homeIdx) {
        if (idx >= devIds.size()) {
            fetchDevicesForHomes(sid, gid, homeIds, homeIdx + 1);
            return;
        }
        final String devId = devIds.get(idx);
        Object[][] post = new Object[][] {{"devId", devId}};
        // Use raw device.get; note probe used version 4.1.
        callDeviceGet(devId, sid, gid, new InitCallback() {
            @Override public void onResult(String origin) {
                showDevice(origin, devId, devNames.get(idx));
                fetchOneDevice(devIds, devNames, idx + 1, sid, gid, homeIds, allHomes, homeIdx);
            }
        });
    }

    // ---- Raw API plumbing (mirrors the probe) ----

    private interface InitCallback { void onResult(String origin); }

    private void callRaw(final String label, final String api, final String version,
            final Object[][] post, final String sid, final Long gid,
            final InitCallback cb) {
        try {
            Class<?> apiParamsCls = Class.forName("com.thingclips.smart.android.base.ApiParams");
            Object params = apiParamsCls.getConstructor(String.class, String.class)
                    .newInstance(api, version);
            Class<?> jsonCls = Class.forName("com.alibaba.fastjson.JSONObject");
            Object json = jsonCls.getConstructor().newInstance();
            if (post != null) {
                Method put = jsonCls.getMethod("put", String.class, Object.class);
                for (Object[] kv : post) put.invoke(json, kv[0], kv[1]);
            }
            apiParamsCls.getMethod("setPostData", jsonCls).invoke(params, json);
            apiParamsCls.getMethod("setSessionRequire", boolean.class).invoke(params, true);
            apiParamsCls.getMethod("setSession", String.class).invoke(params, sid);
            if (gid != null) {
                try { apiParamsCls.getMethod("setGid", long.class).invoke(params, gid.longValue()); }
                catch (Throwable ignore) {}
            }

            Class<?> businessCls = Class.forName("com.thingclips.smart.android.network.Business");
            Object business = businessCls.getConstructor().newInstance();
            Class<?> listenerType = Class.forName("com.thingclips.smart.android.network.Business$ResultListener");
            InvocationHandler lh = new InvocationHandler() {
                @Override public Object invoke(Object proxy, Method method, Object[] args) {
                    String origin = extractOrigin(args);
                    Log.i(TAG, label + " " + method.getName() + " origin=" + origin);
                    if (cb != null) cb.onResult(origin);
                    return null;
                }
            };
            Object listener = Proxy.newProxyInstance(listenerType.getClassLoader(),
                    new Class<?>[]{listenerType}, lh);
            Method req = businessCls.getMethod("asyncRequest",
                    Class.forName("com.thingclips.smart.android.network.ThingApiParams"),
                    Class.class, listenerType);
            req.invoke(business, params, jsonCls, listener);
            Log.i(TAG, "raw " + label + " sent");
        } catch (Throwable e) {
            Log.e(TAG, "callRaw " + label + " failed", e);
            if (cb != null) cb.onResult(null);
        }
    }

    private void callDeviceGet(final String devId, final String sid, final long gid,
            final InitCallback cb) {
        try {
            Class<?> apiParamsCls = Class.forName("com.thingclips.smart.android.base.ApiParams");
            Object params = apiParamsCls.getConstructor(String.class, String.class)
                    .newInstance("thing.m.device.get", "4.1");
            apiParamsCls.getMethod("putPostData", String.class, Object.class)
                    .invoke(params, "devId", devId);
            apiParamsCls.getMethod("setSessionRequire", boolean.class).invoke(params, true);
            apiParamsCls.getMethod("setSession", String.class).invoke(params, sid);
            try { apiParamsCls.getMethod("setGid", long.class).invoke(params, gid); } catch (Throwable ignore) {}

            Class<?> businessCls = Class.forName("com.thingclips.smart.android.network.Business");
            Object business = businessCls.getConstructor().newInstance();
            Class<?> deviceBean = Class.forName("com.thingclips.smart.interior.device.bean.DeviceRespBean");
            Class<?> listenerType = Class.forName("com.thingclips.smart.android.network.Business$ResultListener");
            InvocationHandler lh = new InvocationHandler() {
                @Override public Object invoke(Object proxy, Method method, Object[] args) {
                    String origin = extractOrigin(args);
                    Log.i(TAG, "DEVICE_GET " + method.getName() + " origin=" + origin);
                    if (cb != null) cb.onResult(origin);
                    return null;
                }
            };
            Object listener = Proxy.newProxyInstance(listenerType.getClassLoader(),
                    new Class<?>[]{listenerType}, lh);
            Method req = businessCls.getMethod("asyncRequest",
                    Class.forName("com.thingclips.smart.android.network.ThingApiParams"),
                    Class.class, listenerType);
            req.setAccessible(true);
            req.invoke(business, params, deviceBean, listener);
            Log.i(TAG, "device.get " + devId + " sent");
        } catch (Throwable e) {
            Log.e(TAG, "device.get failed", e);
            if (cb != null) cb.onResult(null);
        }
    }

    private String extractOrigin(Object[] args) {
        if (args == null) return null;
        for (Object a : args) {
            if (a != null && a.getClass().getName().endsWith("BusinessResponse")) {
                try {
                    Object o = getField(a, "originData");
                    if (o instanceof String) return (String) o;
                } catch (Throwable ignore) {}
            }
        }
        return null;
    }

    // ---- JSON parsing + UI ----

    private void parseHomeIds(String origin, List<String> out) {
        if (origin == null) return;
        try {
            JSONObject obj = new JSONObject(origin);
            JSONArray arr = obj.optJSONArray("result");
            if (arr == null) return;
            for (int i = 0; i < arr.length(); i++) {
                JSONObject h = arr.optJSONObject(i);
                if (h == null) continue;
                String id = h.optString("id", "");
                if (!id.isEmpty()) out.add(id);
            }
        } catch (Throwable e) {
            Log.e(TAG, "parse homes failed", e);
        }
    }

    private void parseDeviceIds(String origin, List<String> ids, List<String> names) {
        if (origin == null) return;
        try {
            JSONObject obj = new JSONObject(origin);
            JSONArray arr = obj.optJSONArray("result");
            if (arr == null) return;
            for (int i = 0; i < arr.length(); i++) {
                JSONObject d = arr.optJSONObject(i);
                if (d == null) continue;
                String id = d.optString("id", "");
                if (!id.isEmpty()) {
                    ids.add(id);
                    names.add(d.optString("name", "device"));
                }
            }
        } catch (Throwable e) {
            Log.e(TAG, "parse devices failed", e);
        }
    }

    private void showDevice(String origin, String fallbackId, String fallbackName) {
        String devId = fallbackId;
        String name = fallbackName;
        String localKey = "";
        String productId = "";
        String uuid = "";
        String mac = "";
        if (origin != null) {
            try {
                JSONObject obj = new JSONObject(origin);
                JSONObject result = obj.optJSONObject("result");
                if (result != null) {
                    devId = result.optString("devId", devId);
                    name = result.optString("name", name);
                    localKey = result.optString("localKey", "");
                    productId = result.optString("productId", "");
                    uuid = result.optString("uuid", "");
                    mac = result.optString("mac", "");
                }
            } catch (Throwable e) {
                Log.e(TAG, "parse device failed", e);
            }
        }
        final String fDevId = devId;
        final String fName = name;
        final String fKey = localKey;
        final String fProd = productId;
        final String fUuid = uuid;
        final String fMac = mac;
        Log.i(TAG, "device " + fDevId + " key-found=" + (!fKey.isEmpty()));
        runOnUiThread(new Runnable() {
            @Override public void run() {
                LinearLayout card = new LinearLayout(FinderActivity.this);
                card.setOrientation(LinearLayout.VERTICAL);
                card.setPadding(dp(8), dp(8), dp(8), dp(8));

                TextView title = new TextView(FinderActivity.this);
                title.setText(fName + "  (" + fDevId + ")");
                title.setTextSize(16);
                card.addView(title);

                TextView meta = new TextView(FinderActivity.this);
                meta.setText("product: " + fProd + "\nuuid: " + fUuid + "\nmac: " + fMac);
                meta.setTextSize(12);
                card.addView(meta);

                TextView keyView = new TextView(FinderActivity.this);
                keyView.setText("localKey: " + (fKey.isEmpty() ? "(not returned)" : fKey));
                keyView.setTextSize(15);
                keyView.setTextIsSelectable(true);
                card.addView(keyView);

                if (!fKey.isEmpty()) {
                    Button copy = new Button(FinderActivity.this);
                    copy.setText("Copy key");
                    copy.setOnClickListener(new View.OnClickListener() {
                        @Override public void onClick(View v) {
                            ClipboardManager cm = (ClipboardManager) getSystemService(Context.CLIPBOARD_SERVICE);
                            if (cm != null) {
                                cm.setPrimaryClip(ClipData.newPlainText("localKey", fKey));
                                Toast.makeText(FinderActivity.this, "Key copied", Toast.LENGTH_SHORT).show();
                            }
                        }
                    });
                    card.addView(copy);
                }
                resultsLayout.addView(card);
            }
        });
    }
}
