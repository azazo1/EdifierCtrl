# 运行时

`HeadsetHost` 负责命令节流, BLE 20 字节分片, 以及把接收字节解码成通知.

传输实现:

- `edifier-runtime::MockTransport` 测试用
- `edifier-bt-windows` WinRT RFCOMM / BLE, 音频服务开关与 MMDevice 实际音频端点校验
- `edifier-bt-linux` BlueZ RFCOMM (仅 Linux 编译真实实现)
- `edifier-bt-android` JNI 调 `BluetoothBridge`: 已配对 RFCOMM, A2DP 用隐藏 connect (Android 14 可能失败, 交接回退 CD)
- `edifier-bt-macos` dlsym Swift `@_cdecl` IOBluetooth 桥, CoreAudio 确认蓝牙输出端点, 显式接管时选择默认输出. 详见 [macOS 桌面端](macos.md).
- Linux 音频走 BlueZ D-Bus, 根据 MediaTransport 和设备连接状态确认音频及释放, 不将普通 ACL 连接当作 A2DP 成功.

Windows 优先 RFCOMM. BLE 随机 MAC 时 `FromBluetoothAddressAsync` 可能失败, 需要本机已用经典蓝牙配对.

`suppress_autoreconnect` 在各系统上受公开 API 限制. Windows 记录并恢复本次修改前的音频服务状态, Linux 和 macOS 不能保证独立抑制系统重连. 交接必须确认实际释放, 无法确认时中止并报告失败. CD 回退只作用于地址匹配的控制通道, 不跨耳机发送.

组成员使用本地心跳期限过滤离线设备. `group_leave` 停止发现与交接任务并恢复可恢复的重连抑制状态, 不主动中断用户正在使用的本机音频.

局域网见 `docs/group.md`. `just listen <口令>` 听组员宣告. `--connect <MAC>` 认领 A2DP, `--claim-peer <id>` 向该成员请求接管.
