exe := "apps/windows-winui/bin/x64/Debug/net8.0-windows10.0.19041.0/EdifierCtrl.exe"

[private]
default:
    @just --list windows

# 编 FFI DLL.
ffi:
    cargo build -p edifier-ffi

# 编非打包 WinUI 3.
build: ffi
    dotnet build apps/windows-winui/EdifierCtrl.csproj -c Debug -p:Platform=x64

# 启动 WinUI.
run: build
    {{exe}}
