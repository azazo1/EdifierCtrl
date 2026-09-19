apk := "apps/android/app/build/outputs/apk/debug/app-debug.apk"
sdk-fallback := env_var_or_default("LOCALAPPDATA", env_var_or_default("HOME", "")) / "Android" / "Sdk"
export ANDROID_HOME := env_var_or_default("ANDROID_HOME", env_var_or_default("ANDROID_SDK_ROOT", sdk-fallback))
export ANDROID_SDK_ROOT := ANDROID_HOME
adb := if ANDROID_HOME != "" { ANDROID_HOME / "platform-tools" / "adb" } else { "adb" }

[private]
default:
    @just --list android

# 交叉编 arm64 so 到 jniLibs.
ffi:
    cargo ndk -t arm64-v8a -o apps/android/app/src/main/jniLibs build -p edifier-ffi --release

# 编 so 和 debug APK.
build: ffi
    cd apps/android && gradle assembleDebug

# 编好后装到已连接的设备.
install: build
    "{{adb}}" install -r {{apk}}

# 安装并启动.
run: install
    "{{adb}}" shell am start -n dev.edifierctrl.app/.MainActivity
