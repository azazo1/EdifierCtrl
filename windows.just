[private]
default:
    @just --list windows

# 编译 Rust FFI 动态库.
ffi:
    cargo build -p edifier-ffi --locked

# 构建 WinUI 桌面应用.
build:
    pwsh -NoProfile -File apps/windows-winui/Packaging/package.ps1 build

# 构建并启动 WinUI.
run:
    pwsh -NoProfile -File apps/windows-winui/Packaging/package.ps1 run

# 使用独立数据目录和详细日志启动.
debug:
    pwsh -NoProfile -File apps/windows-winui/Packaging/package.ps1 debug

# 构建自包含的 Windows x64 便携包.
dist:
    pwsh -NoProfile -File apps/windows-winui/Packaging/package.ps1 dist
