# 协议摘要

发送帧: `AA + len + body + checksum`. 校验和初值 8217, 对校验和之前的字节累加, 追加 16-bit 大端.

接收帧头为 `BB` (响应) 或 `CC` (确认), 总长 `len + 4`.

RFCOMM UUID: `edf00000-edfe-dfed-fedf-edfedfedfedf`.

BLE 写特征按 20 字节切片, 间隔 50ms. 多数机型 service UUID 带后缀 `-1a48-11e9-ab14-d663bd873d93`.

## 常用命令

| 动作 | body |
| --- | --- |
| 标准 / 降噪 / 通透 | `C101` / `C102` / `C103` |
| 通透音量 | `C103` + `(6 + vol)` , vol 为 `-3..=3` |
| 音效 | `C400`..`C403` |
| 游戏模式 | `0900` / `0901` |
| LDAC | `4900` / `4901` / `4902` |
| 断开主机 | `CD` |
| 关机 / 配对 / 复位 | `CE` / `CF` / `07` |
| 电量 / MAC / 固件 | `D0` / `C8` / `C6` |

破坏性命令 (`CD` `CE` `CF` `07`) 必须经用户确认. 交接自动化只用 `CD` 作为回退.

设置 JSON 与 mEDIFIER 兼容: `{ name, commands: [{ cmd, name, priority }] }`, 另增 `version`.
