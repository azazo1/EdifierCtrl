#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
cd "$root"
mode="${1:-build}"
[[ "$#" -le 1 ]] || { printf '用法: bash apps/macos/Packaging/package.sh [build|run|debug|dist|fake-dist]\n' >&2; exit 64; }
[[ "$(uname -s)" == Darwin ]] || { printf '此打包脚本需要 macOS.\n' >&2; exit 1; }
configuration=debug
version=dev-build
fake=false
case "$mode" in
    build|run|debug) ;;
    dist) configuration=release; version="${EDIFIER_BUILD_VERSION:-$(bash apps/macos/Packaging/build-version.sh)}" ;;
    fake-dist) configuration=release; version=v0.0.0; fake=true ;;
    *) printf '未知构建模式: %s\n' "$mode" >&2; exit 64 ;;
esac
[[ "$version" =~ ^[A-Za-z0-9.+^-]+$ ]] || { printf '构建版本包含不安全字符.\n' >&2; exit 1; }
case "$(uname -m)" in
    arm64) architecture=aarch64 ;;
    x86_64) architecture=x86_64 ;;
    *) printf '不支持当前 CPU 架构.\n' >&2; exit 1 ;;
esac

repository="${EDIFIER_RELEASE_REPOSITORY:-}"
if [[ -z "$repository" ]]; then
    remote="$(git remote get-url origin 2>/dev/null || true)"
    case "$remote" in
        https://github.com/*) repository="${remote#https://github.com/}" ;;
        git@github.com:*) repository="${remote#git@github.com:}" ;;
        ssh://git@github.com/*) repository="${remote#ssh://git@github.com/}" ;;
    esac
    repository="${repository%.git}"
fi
if [[ -n "$repository" && ! "$repository" =~ ^[A-Za-z0-9][A-Za-z0-9_.-]*/[A-Za-z0-9][A-Za-z0-9_.-]*$ ]]; then
    printf 'EDIFIER_RELEASE_REPOSITORY 必须是 GitHub owner/repo.\n' >&2
    exit 1
fi
if [[ -z "$repository" ]]; then printf '未配置公开 Release 仓库, 应用内更新会显示未配置状态.\n' >&2; fi

printf '[1/5] 构建 Rust 动态库 (%s)\n' "$configuration"
cargo_args=(build --locked -p edifier-ffi)
if [[ "$configuration" == release ]]; then cargo_args+=(--release); fi
cargo "${cargo_args[@]}"
printf '[2/5] 构建 SwiftUI 应用\n'
swift build --package-path apps/macos --configuration "$configuration"
bin_dir="$(swift build --package-path apps/macos --configuration "$configuration" --show-bin-path)"

printf '[3/5] 组装应用包, 版本 %s\n' "$version"
output="$root/target/macos/$configuration"
if [[ "$mode" == dist || "$mode" == fake-dist ]]; then output="$root/target/macos/$mode"; fi
mkdir -p "$output"
work="$(mktemp -d "$output/.package-XXXXXXXX")"
mount_dir=""
cleanup() {
    if [[ -n "$mount_dir" ]]; then hdiutil detach "$mount_dir" || true; fi
    rm -rf -- "$work"
}
trap cleanup EXIT
app="$work/EdifierCtrl.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Frameworks" "$app/Contents/Resources"
cp "$bin_dir/EdifierCtrl" "$app/Contents/MacOS/EdifierCtrl"
cp "$root/target/$configuration/libedifier_ffi.dylib" "$app/Contents/Frameworks/libedifier_ffi.dylib"
cp apps/macos/Packaging/Info.plist "$app/Contents/Info.plist"
cp apps/macos/Packaging/apply-update.sh "$app/Contents/Resources/apply-update.sh"
swift apps/macos/Packaging/generate-icon.swift "$app/Contents/Resources/AppIcon.icns"
chmod 755 "$app/Contents/MacOS/EdifierCtrl" "$app/Contents/Frameworks/libedifier_ffi.dylib" "$app/Contents/Resources/apply-update.sh"
/usr/libexec/PlistBuddy -c "Set :EdifierBuildVersion $version" "$app/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Set :EdifierFakeBuild $fake" "$app/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Set :EdifierReleaseRepository $repository" "$app/Contents/Info.plist"
base_version="${version#v}"
base_version="${base_version%%[-+^]*}"
if [[ "$base_version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    /usr/libexec/PlistBuddy -c "Set :CFBundleShortVersionString $base_version" "$app/Contents/Info.plist"
fi
install_name_tool -id '@rpath/libedifier_ffi.dylib' "$app/Contents/Frameworks/libedifier_ffi.dylib"
# 原生桥从 Contents/Frameworks dlopen, 其 Swift @_cdecl 符号由 export_dynamic 导出.
plutil -lint "$app/Contents/Info.plist"
file "$app/Contents/MacOS/EdifierCtrl" "$app/Contents/Frameworks/libedifier_ffi.dylib"
for binary in "$app/Contents/MacOS/EdifierCtrl" "$app/Contents/Frameworks/libedifier_ffi.dylib"; do
    [[ "$(file -b "$binary")" == *Mach-O* ]] || { printf '无效 Mach-O 文件: %s\n' "$binary" >&2; exit 1; }
done

printf '[4/5] 签名并校验 bundle\n'
identity="${EDIFIER_CODESIGN_IDENTITY:--}"
sign_options=(--force --sign "$identity")
if [[ "$identity" != - ]]; then sign_options+=(--options runtime --timestamp); fi
codesign "${sign_options[@]}" "$app/Contents/Frameworks/libedifier_ffi.dylib"
codesign "${sign_options[@]}" "$app"
codesign --verify --deep --strict "$app"
if [[ "$identity" == - ]]; then printf '使用本地 ad-hoc 签名. 公开发行前需 Developer ID 签名与公证.\n'; fi

final_app="$output/EdifierCtrl.app"
[[ ! -L "$final_app" ]] || { printf '拒绝覆盖符号链接应用包.\n' >&2; exit 1; }
if [[ -e "$final_app" ]]; then rm -rf -- "$final_app"; fi
mv "$app" "$final_app"
if [[ "$mode" == dist || "$mode" == fake-dist ]]; then
    printf '[5/5] 创建并校验 DMG\n'
    image_root="$work/image"
    mkdir "$image_root"
    ditto "$final_app" "$image_root/EdifierCtrl.app"
    ln -s /Applications "$image_root/Applications"
    suffix=""
    if [[ "$fake" == true ]]; then suffix=-fake; fi
    filename="EdifierCtrl-$version-macos-$architecture$suffix.dmg"
    dmg="$output/$filename"
    hdiutil create -volname EdifierCtrl -srcfolder "$image_root" -format UDZO -ov "$dmg"
    mount_dir="$work/verify"
    mkdir "$mount_dir"
    hdiutil attach -readonly -nobrowse -noautoopen -mountpoint "$mount_dir" "$dmg"
    [[ -x "$mount_dir/EdifierCtrl.app/Contents/MacOS/EdifierCtrl" ]]
    [[ -f "$mount_dir/EdifierCtrl.app/Contents/Frameworks/libedifier_ffi.dylib" ]]
    [[ "$(readlink "$mount_dir/Applications")" == /Applications ]]
    codesign --verify --deep --strict "$mount_dir/EdifierCtrl.app"
    hdiutil detach "$mount_dir"
    mount_dir=""
    # 此本地清单只包含本次构建. Release 汇总时需合并所有架构产物.
    (cd "$output" && shasum -a 256 "$filename" >SHA256SUMS)
    printf '应用: %s\n安装包: %s\n校验和: %s/SHA256SUMS\n' "$final_app" "$dmg" "$output"
else
    printf '[5/5] 应用包已就绪: %s\n' "$final_app"
fi
if [[ "$mode" == debug ]]; then
    export EDIFIER_DATA_DIR="$root/target/edifierctrl-debug"
    export EDIFIER_LOG_FILE="$EDIFIER_DATA_DIR/app.log"
    export EDIFIER_LOG_LEVEL=debug
    mkdir -p "$EDIFIER_DATA_DIR"
    "$final_app/Contents/MacOS/EdifierCtrl"
elif [[ "$mode" == run ]]; then
    "$final_app/Contents/MacOS/EdifierCtrl"
fi
