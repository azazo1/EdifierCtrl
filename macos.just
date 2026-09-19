dylib := "target/debug/libedifier_ffi.dylib"

[private]
default:
    @just --list macos

# 编 FFI dylib.
ffi:
    cargo build -p edifier-ffi

# 编 SwiftUI 并把 dylib 放到可执行文件旁.
build: ffi
    cd apps/macos && swift build
    cp {{dylib}} apps/macos/libedifier_ffi.dylib

# 运行 macOS 端.
run: build
    cd apps/macos && swift run
