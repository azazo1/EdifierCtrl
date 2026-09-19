# EdifierCtrl

漫步者耳机的开源控制端. 核心协议做成 Rust 库, 四个操作系统各自用原生界面, 局域网组内可以交接蓝牙音频.

协议与机型行为参考了 [mEDIFIER](https://github.com/wh201906/mEDIFIER) (MIT). 本仓库不依赖也不分发那份代码.

## 现状

核心库, C ABI 会话, Windows / Linux / Android / macOS 蓝牙适配和局域网交接已接通. Android 提供对应桌面交互的五页工作台, 用 `just android build` 构建; 说明见 [Android 客户端](docs/android.md). macOS 和 Windows 提供设备工作台, 后台驻留与交接进度; 分别用 `just macos build` 和 `just windows build` 构建. 使用说明见 [macOS 桌面端](docs/macos.md) 和 [Windows 桌面端](docs/windows.md).

- `edifier-protocol` / `edifier-session` / `edifier-group` / `edifier-runtime`
- `edifier-ffi`: C ABI, 头文件 `crates/edifier-ffi/include/edifier.h`, 说明见 `docs/ffi.md`
- `edifier-bt-windows` / `edifier-bt-linux` / `edifier-bt-android` / `edifier-bt-macos`: 平台蓝牙
- `edifier-cli`: 编解码与扫描
- `apps/windows-winui`: WinUI 3
- `apps/android`: Jetpack Compose
- `apps/linux-gtk`: GTK4 (独立 Cargo 包, 不进 workspace)
- `apps/macos`: SwiftUI

首批机型: W820NB, W820NB Double Gold, W200BT Plus, Generic Device.

## 命令

```shell
just test
just clippy
just ffi
just scan
just listen mypass
just encode C101
just decode AA02C1012187
just parse BB02D04D21F3
just profiles
just android build
just android install
just android run
just windows build
just windows run
just macos build
just macos run
just linux build
just linux run
```

## 结构

```
crates/   核心库, CLI, FFI
apps/     四端原生应用
docs/     协议, 组网, FFI
```

设置文件带 `version` 字段. 无版本的 mEDIFIER JSON 会按版本 0 迁移.

## 限制

耳机需先在系统里配对. 局域网交接不走公网. Android 音频连接依赖系统隐藏 API, macOS 可能抢着重连, Windows BLE 仍受随机 MAC 影响.
