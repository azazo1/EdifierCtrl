# FFI 合同

各端 UI 只通过 C ABI 调核心库. 头文件: `crates/edifier-ffi/include/edifier.h`.

动态库名: `edifier_ffi` (Windows 为 `edifier_ffi.dll`).

## 字符串

返回的 `char*` 是 UTF-8, 调用方必须 `edifier_string_free`.
失败返回 `NULL`, 随后 `edifier_last_error` 给出原因 (线程局部, 不要 free).

## 命令 JSON

`edifier_command_encode` 入参带 `op` 字段, 例如:

```json
{"op":"set_noise_mode","mode":"normal"}
{"op":"set_ldac","mode":"rate96k"}
{"op":"disconnect_host"}
{"op":"raw","hex":"CD"}
```

返回:

```json
{"hex":"AA02C1012187","body":"C101","destructive":false,"label":"Noise Reduction","op":{...}}
```

`destructive` 为 true 的命令 (`disconnect_host`, `power_off`, `re_pair`, `factory_reset`) 必须在 UI 里二次确认. 交接回退可以自动发 `disconnect_host`.

## 通知 JSON

`edifier_frame_parse` / `edifier_decoder_push_hex` 返回 `kind` 字段, 例如 `battery`, `noise`, `mac`.

## 交接动作 JSON

数组, 每项带 `action`:

- `send` 带 `message` (组报文, 与 `docs/group.md` 相同)
- `connect_audio` / `disconnect_audio` 带 `mac`
- `suppress_autoreconnect` 带 `mac` 和 `suppress`
- `send_headset_disconnect` 表示发 `CD`
- `report` 带 `progress.kind`

## 机型

`edifier_profiles_json` 给出 `features` 白名单. UI 只渲染列表里有的功能.

## 会话

`edifier_session_new` 打开平台蓝牙和后台 pump. `scan` / `connect` / `send_json` / `readout` 走 `HeadsetHost`. `connect` 成功后记下耳机地址, `group_join` 或之后会把它写入组 Announce 的 `holding`. `group_claim` 对耳机 MAC 发起交接. `group_claim_peer` 用成员 `id` 取其 `holding` 再 claim, 对应 UI 里点某个主机.

macOS 应用用 dlopen 加载 `libedifier_ffi.dylib`, 把 dylib 放在可执行文件旁即可. 同一进程里 Swift 导出 `edifier_macos_*`, Rust 用 `RTLD_DEFAULT` 找到 IOBluetooth 桥.

`poll_event` 返回 `{"kind":"empty"}` 或 `bt_state` / `headset` / `peer` / `handoff`.

Android JNI 符号在 `dev.edifierctrl.app.EdifierNative`. 用 `cargo ndk` 编 `aarch64-linux-android` 后把 `libedifier_ffi.so` 放到 `apps/android/app/src/main/jniLibs/arm64-v8a/`.
