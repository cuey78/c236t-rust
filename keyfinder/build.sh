#!/bin/bash
set -e
JAVAC=javac
ANDROID_JAR=/tmp/opencode/android-tools/android-34/android.jar
D8=/tmp/opencode/android-tools/android-16/d8
AAPT=/tmp/opencode/android-tools/android-16/aapt
ZIPALIGN=/tmp/opencode/android-tools/android-16/zipalign
APKSIGNER=/tmp/opencode/android-tools/android-16/apksigner
ROOT=/tmp/opencode/keyfinder
STAGE=/tmp/opencode/probe/stage
PKG=$ROOT/package

rm -rf $ROOT/classes/* $ROOT/dexes/*
$JAVAC -source 8 -target 8 -cp "$ANDROID_JAR" -d $ROOT/classes $ROOT/src/com/c236t/keyfinder/FinderActivity.java
$D8 --release --lib "$ANDROID_JAR" --output $ROOT/dexes $ROOT/classes/com/c236t/keyfinder/*.class
cp $ROOT/dexes/classes.dex $STAGE/classes2.dex.tmp
# classes2.dex slot is proven for the probe; use it for the finder too.
cp $ROOT/dexes/classes.dex $STAGE/classes2.dex

cd $PKG
rm -f keyfinder-aligned.apk keyfinder-base.apk
$AAPT package -f -M AndroidManifest.xml -I "$ANDROID_JAR" -F keyfinder-base.apk --auto-add-overlay
python3 - <<'EOF'
import zipfile,os
base='/tmp/opencode/keyfinder/package/keyfinder-base.apk'; tmp='/tmp/opencode/keyfinder/package/unsigned.apk'
# Never ship the old hard-coded probe (it contains a real password).
EXCLUDE = {'classes29.dex'}
z=zipfile.ZipFile(base); entries={n:z.read(n) for n in z.namelist()}
for root,dirs,files in os.walk('/tmp/opencode/probe/stage'):
 for f in files:
  if f in EXCLUDE: continue
  p=os.path.join(root,f); arc=os.path.relpath(p,'/tmp/opencode/probe/stage'); entries[arc]=open(p,'rb').read()
with zipfile.ZipFile(tmp,'w',zipfile.ZIP_DEFLATED) as out:
 for name,data in entries.items(): out.writestr(name,data)
os.remove(base); os.rename(tmp,base)
EOF
$ZIPALIGN -p 4 keyfinder-base.apk keyfinder-aligned.apk
$APKSIGNER sign --ks $ROOT/debug.keystore --ks-pass pass:android --key-pass pass:android keyfinder-aligned.apk
echo "built: $PKG/keyfinder-aligned.apk"
ls -lh $PKG/*.apk
rm -f $STAGE/classes2.dex.tmp
