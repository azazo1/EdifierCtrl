[private]
default:
    @just --list linux

# 编 GTK4 端.
build:
    cargo build --manifest-path apps/linux-gtk/Cargo.toml

# 运行 Linux GTK4 端.
run:
    cargo run --manifest-path apps/linux-gtk/Cargo.toml
