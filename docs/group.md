# 局域网组与音频交接

组口令派生 16 字节 `group_id` 和 32 字节 HMAC-SHA256 密钥. 报文带版本, 时间戳和 MAC, 时差超过 30s 丢弃. 不走公网.

UDP 组播 `239.18.70.17:18721`. 外层 `Datagram.from` 用来丢掉本机回环. 进程内测试走 `LoopbackNet`.

交接对象是一副已配对耳机的 MAC. 组员是运行本应用的主机. 控制通道连上后会话会 `adopt_headset`: 记下 MAC, 尽量连上 A2DP 并标 `has_audio`. 否则对端 claim 时持有方会回 `HandoffNoAudio`, 本机 A2DP 不会断. UI 点某个成员时走 `group_claim_peer`, 取其当前 `holding`.

1. 请求端广播 `HandoffRequest`.
2. 持有音频的成员抑制自动重连并断开 A2DP, 回复 `Released`.
3. 请求端连接音频, 回复 `Taken`.
4. 若无人持有或超时: 请求端若已有 BLE/RFCOMM 控制, 发 `CD`, 等 2.5s 再连.
5. 同一耳机同时只允许一个 nonce; 冲突回 `Busy`.

`GroupHub` 把状态机接到 `AudioControl` 和可选的 `CdFallback`.
