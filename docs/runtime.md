# 运行时

`HeadsetHost` 负责命令节流, BLE 20 字节分片, 以及把接收字节解码成通知.

传输实现:

- `edifier-runtime::MockTransport` 测试用
- `edifier-bt-windows` WinRT RFCOMM / BLE / AudioPlaybackConnection
- `edifier-bt-linux` BlueZ RFCOMM (仅 Linux 编译真实实现)
- `edifier-bt-android` JNI 调 `BluetoothBridge`: 已配对 RFCOMM, A2DP 用隐藏 connect (Android 14 可能失败, 交接回退 CD)
- `edifier-bt-macos` dlsym Swift `@_cdecl` IOBluetooth 桥: 已配对 RFCOMM, 音频用 `openConnection`/`closeConnection` (系统可能自动重连)
- Linux 音频走 `bluetoothctl connect` / `disconnect`

Windows 优先 RFCOMM. BLE 随机 MAC 时 `FromBluetoothAddressAsync` 可能失败, 需要本机已用经典蓝牙配对.

`suppress_autoreconnect` 在 Windows / Linux 都没有稳定公开接口, 交接时只能尽量断开 AudioPlaybackConnection / A2DP, 再发 `CD`.

局域网见 `docs/group.md`. `just listen <口令>` 听组员宣告. `--connect <MAC>` 认领 A2DP, `--claim-peer <id>` 向该成员请求接管.
