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

`edifier_session_new` 创建会话, 成功连接控制通道后启动接收 pump. `scan` / `connect` / `send_json` / `readout` 走 `HeadsetHost`. `connect` 记录待观察的耳机地址, 只有平台确认实际音频连接后才在组 Announce 中宣告 `holding`. `group_claim` 对耳机 MAC 发起交接. `group_claim_peer` 用成员 `id` 取其 `holding` 再 claim, 对应 UI 里点某个主机.

`edifier_session_disconnect` 仅关闭控制通道, 不自动断开系统音频. `edifier_session_group_leave` 幂等退出组, 取消组后台任务和交接, 保留本机控制. `edifier_session_holding` 返回已确认持有的耳机地址, 未确认时返回空字符串. `edifier_session_free` 停止后台任务并释放控制通道, 调用方必须确保此后不再使用该句柄.

macOS 的全部阻塞 FFI 调用均在独立串行队列执行, 不得在主线程同步等待, 包括会话释放. Swift 的 IOBluetooth 操作和 delegate 依赖主线程 run loop. 应用包从 `Contents/Frameworks/libedifier_ffi.dylib` 加载核心库, 裸二进制也可从同目录加载. Swift 导出 `edifier_macos_*`, Rust 用 `RTLD_DEFAULT` 找到原生桥, 可执行文件需要导出这些符号.

`poll_event` 返回 `{"kind":"empty"}` 或 `bt_state` / `headset` / `audio` / `peer` / `handoff` / `message`. 控制连接, 系统音频与交接进度是不同状态, UI 不应互相推断成功.

Android JNI 符号在 `dev.edifierctrl.app.EdifierNative`. 用 `cargo ndk` 编 `aarch64-linux-android` 后把 `libedifier_ffi.so` 放到 `apps/android/app/src/main/jniLibs/arm64-v8a/`.
