[private]
default:
    @just --list

# Android: just android build|install|run
mod android

# Windows: just windows build|run
mod windows

# just win 等同 just windows
mod win 'windows.just'

# macOS: just macos build|run
mod macos

# Linux: just linux build|run
mod linux

# 运行工作区测试.
test:
    cargo test --workspace

# 检查 clippy.
clippy:
    cargo clippy --workspace --all-targets -- -D warnings

# 构建 C ABI 动态库.
ffi:
    cargo build -p edifier-ffi

# just encode C101
# 把命令载荷封装成发送帧.
encode payload:
    cargo run -p edifier-cli -- encode {{payload}}

# just decode AA02C1012187
# 解开完整收发帧.
decode frame:
    cargo run -p edifier-cli -- decode {{frame}}

# just parse BB02D04D21F3
# 解开帧并解析通知.
parse frame:
    cargo run -p edifier-cli -- parse {{frame}}

# 列出内置机型档案.
profiles:
    cargo run -p edifier-cli -- profiles

# just scan
# just scan ble
# 扫描已配对耳机.
scan kind="rfcomm":
    cargo run -p edifier-cli -- scan --kind {{kind}}

# just listen mypass
# just listen mypass 12
# just listen mypass 20 --connect AA:BB:CC:DD:EE:FF
# just listen mypass 20 --claim-peer other-host
# 加入局域网组, 可选连耳机或向成员请求接管.
listen passphrase seconds="8" *args:
    cargo run -p edifier-cli -- listen {{passphrase}} --seconds {{seconds}} {{args}}
