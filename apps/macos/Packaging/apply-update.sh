#!/usr/bin/env bash
set -euo pipefail
trap '' HUP INT

# 所有路径来自独立参数, 不执行调用方传入的 shell 文本.
[[ "$#" -eq 11 ]] || exit 64
old_pid="$1"
bundle="$2"
staging="$3"
backup="$4"
archive="$5"
expected_digest="$6"
expected_version="$7"
update_dir="$8"
expected_id="$9"
final_data_dir="${10}"
final_log_file="${11}"
[[ "$final_data_dir" == /* && "$final_log_file" == /* ]] || exit 65

[[ "$update_dir" == /* && -d "$update_dir" && ! -L "$update_dir" ]] || exit 65
[[ ! -L "$update_dir/apply-update.log" && ! -L "$update_dir/apply-update-result.txt" && ! -L "$update_dir/apply-update.pid" ]] || exit 65
exec >>"$update_dir/apply-update.log" 2>&1
printf '%s\n' "$$" >"$update_dir/apply-update.pid"
printf '[%s] 开始安装更新 %s\n' "$(date -u '+%FT%TZ')" "$expected_version"
validated=false
moved_old=false
moved_new=false
success=false
old_exited=false
mount_dir=""
staging_created=false
reason="安装助手遇到错误. 请查看 apply-update.log, 退出应用后手动拖拽安装 DMG."

cleanup() {
    local result=$?
    trap - EXIT
    if [[ "$success" != true ]]; then
        printf '[%s] 更新失败: %s (exit %s)\n' "$(date -u '+%FT%TZ')" "$reason" "$result"
        printf '%s\n' "$reason" >"$update_dir/apply-update-result.txt"
        if [[ "$validated" == true && "$moved_old" == true ]]; then
            if [[ "$moved_new" == true && -d "$bundle" && ! -L "$bundle" ]]; then
                /bin/mv "$bundle" "$staging" || true
            fi
            if [[ ! -e "$bundle" && -d "$backup" && ! -L "$backup" ]]; then
                /bin/mv "$backup" "$bundle" || true
            fi
        fi
        if [[ "$validated" == true && "$old_exited" == true && -d "$bundle" && ! -L "$bundle" ]]; then
            /usr/bin/nohup "$bundle/Contents/MacOS/EdifierCtrl" </dev/null &
        fi
    fi
    if [[ -n "$mount_dir" ]]; then
        /usr/bin/hdiutil detach "$mount_dir" || true
        /bin/rmdir "$mount_dir" || true
    fi
    if [[ "$validated" == true && "$staging_created" == true && -d "$staging" && ! -L "$staging" ]]; then
        /bin/rm -rf -- "$staging" || true
    fi
    /bin/rm -f -- "$update_dir/apply-update.pid"
    exit "$result"
}
trap cleanup EXIT
fail() { reason="$1"; exit 1; }

[[ "$old_pid" =~ ^[1-9][0-9]*$ && "$old_pid" -gt 1 ]] || fail '旧进程 PID 无效, 未修改应用.'
[[ "$expected_digest" =~ ^[a-fA-F0-9]{64}$ ]] || fail '安装包摘要无效, 未修改应用.'
[[ "$bundle" == /*.app && -d "$bundle" && ! -L "$bundle" ]] || fail '更新目标不是有效的应用 bundle, 未修改应用.'
parent="$(/usr/bin/dirname "$bundle")"
name="$(/usr/bin/basename "$bundle")"
[[ "$parent" != / && "$(cd "$parent" && pwd -P)" == "$parent" ]] || fail '应用父目录不是规范路径, 未修改应用.'
[[ -f "$bundle/Contents/Info.plist" && -x "$bundle/Contents/MacOS/EdifierCtrl" ]] || fail '目标应用结构无效, 未修改应用.'
[[ "$(/usr/bin/dirname "$staging")" == "$parent" && "$staging" == "$parent/.$name.update-"* ]] || fail '暂存路径必须与应用位于同一目录, 未修改应用.'
[[ "${staging#"$parent/.$name.update-"}" =~ ^[A-Za-z0-9-]+$ ]] || fail '暂存路径格式无效, 未修改应用.'
[[ "$backup" == "$bundle.old" && "$(/usr/bin/dirname "$backup")" == "$parent" ]] || fail '备份路径必须由应用 bundle 推导, 未修改应用.'
[[ "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$bundle/Contents/Info.plist")" == "$expected_id" ]] || fail '当前应用标识不一致, 未修改应用.'
validated=true

printf '[%s] 等待旧进程退出\n' "$(date -u '+%FT%TZ')"
for ((attempt=0; attempt<60; attempt++)); do
    if ! /bin/kill -0 "$old_pid" 2>/dev/null; then old_exited=true; break; fi
    /bin/sleep 1
done
[[ "$old_exited" == true ]] || fail '旧应用未在 60 秒内退出, 本次没有替换或打开 DMG. 请稍后重新发起更新.'
[[ ! -e "$staging" && ! -L "$staging" && ! -e "$backup" && ! -L "$backup" ]] || fail '发现已有暂存或备份, 为保护文件已中止. 请查看安装日志后手动安装.'
[[ -w "$parent" && -w "$bundle" ]] || fail '应用所在目录不可写. 请退出应用后, 打开 DMG 手动拖拽安装.'
[[ "$archive" == "$update_dir/"*.dmg && -f "$archive" && ! -L "$archive" ]] || fail '安装包路径无效, 请重新下载.'
printf '[%s] 再次校验安装包\n' "$(date -u '+%FT%TZ')"
actual_digest="$(/usr/bin/shasum -a 256 "$archive")"
actual_digest="${actual_digest%% *}"
[[ "$actual_digest" == "$expected_digest" ]] || fail '安装包摘要发生变化, 未修改应用. 请重新下载.'
mount_dir="$(/usr/bin/mktemp -d "$update_dir/mount-XXXXXXXX")"
reason='无法挂载安装包. 请查看安装日志, 退出应用后手动安装.'
/usr/bin/hdiutil attach -readonly -nobrowse -noautoopen -mountpoint "$mount_dir" "$archive"
source_app="$mount_dir/EdifierCtrl.app"
[[ -d "$source_app" && ! -L "$source_app" && -x "$source_app/Contents/MacOS/EdifierCtrl" ]] || fail '镜像缺少有效的 EdifierCtrl.app, 请手动检查 Release.'
[[ "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$source_app/Contents/Info.plist")" == "$expected_id" ]] || fail '新应用的 bundle id 与当前应用不一致.'
[[ "$(/usr/libexec/PlistBuddy -c 'Print :EdifierBuildVersion' "$source_app/Contents/Info.plist")" == "$expected_version" ]] || fail '新应用版本与 Release tag 不一致.'
[[ "$(/usr/libexec/PlistBuddy -c 'Print :EdifierFakeBuild' "$source_app/Contents/Info.plist")" == false ]] || fail 'Release 中包含 fake 构建, 已拒绝安装.'
reason='安装包签名或内容校验失败. 请从 Release 页面重新下载安装包.'
/usr/bin/codesign --verify --deep --strict "$source_app"
printf '[%s] 复制新应用到同卷暂存目录\n' "$(date -u '+%FT%TZ')"
reason='复制新应用失败. 原应用保持可用, 请退出后手动安装.'
staging_created=true
/usr/bin/ditto "$source_app" "$staging"
if ! /usr/bin/xattr -dr com.apple.quarantine "$staging"; then
    printf '未能清除部分 quarantine 属性, 将继续执行签名校验.\n'
fi
/usr/bin/codesign --verify --deep --strict "$staging"
reason='替换应用失败, 已尝试恢复原应用. 请查看 apply-update.log.'
/bin/mv "$bundle" "$backup"
moved_old=true
/bin/mv "$staging" "$bundle"
moved_new=true
printf '[%s] 替换完成, 正在重新启动\n' "$(date -u '+%FT%TZ')"
reason='新应用启动失败, 已尝试回滚原应用. 请查看 apply-update.log.'
# 传最终数据路径, 让 fake 更新成正式 bundle 后本次重启仍使用隔离数据.
/usr/bin/nohup /usr/bin/env "EDIFIER_DATA_DIR=$final_data_dir" "EDIFIER_LOG_FILE=$final_log_file" "$bundle/Contents/MacOS/EdifierCtrl" </dev/null &
new_pid=$!
/bin/sleep 2
/bin/kill -0 "$new_pid"
success=true
# 备份保留到新应用启动时清理, 避免启动前丢失可回退副本.
printf '[%s] 安装完成\n' "$(date -u '+%FT%TZ')"
