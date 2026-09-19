[private]
default:
    @just --list macos

# 构建 macOS C ABI 动态库.
ffi:
    cargo build --locked -p edifier-ffi

# 构建可从 Finder 启动的 macOS .app.
build:
    bash apps/macos/Packaging/package.sh build

# 构建并在终端前台运行 macOS 应用, 支持 Ctrl+C 优雅退出.
run:
    bash apps/macos/Packaging/package.sh run

# 使用 target/edifierctrl-debug 隔离配置和日志运行应用.
debug:
    bash apps/macos/Packaging/package.sh debug

# 根据当前 macOS 架构生成 .app, DMG 和 SHA256SUMS.
dist:
    bash apps/macos/Packaging/package.sh dist

# 使用相同打包流程生成数据隔离的 v0.0.0 更新测试包.
fake-dist:
    bash apps/macos/Packaging/package.sh fake-dist
