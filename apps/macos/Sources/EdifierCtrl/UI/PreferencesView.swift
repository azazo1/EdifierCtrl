import AppKit
import SwiftUI

struct PreferencesView: View {
    @ObservedObject var model: AppModel
    @ObservedObject private var preferences = AppPreferences.shared
    @ObservedObject private var updates = UpdateManager.shared

    var body: some View {
        VStack(alignment: .leading, spacing: 22) {
            Surface {
                VStack(alignment: .leading, spacing: 22) {
                    SectionHeading(title: "后台与窗口", detail: "关闭主窗口后, 耳机服务仍在菜单栏运行.")
                    preference("启动时隐藏主窗口", detail: "启动后只显示菜单栏图标", value: $preferences.startHidden)
                    Divider()
                    preference("隐藏窗口时隐藏 Dock 图标", detail: "点击菜单栏图标可恢复主窗口", value: $preferences.hideDockWhenHidden)
                    Divider()
                    preference("启动时自动加入交接组", detail: "需要先在交接页记住组名", value: $preferences.autoJoinGroup)
                }
            }
            Surface {
                VStack(alignment: .leading, spacing: 20) {
                    SectionHeading(title: "版本与更新", detail: "EdifierCtrl \(AppVersion.display)")
                    preference("自动检查新版本", detail: "更新下载完成后, 由你决定何时安装", value: $preferences.autoCheckUpdates)
                    HStack(spacing: 12) {
                        Button("检查更新") { updates.isPresented = true; updates.checkForUpdates(userInitiated: true) }
                            .buttonStyle(ActionStyle())
                        if updates.hasUpdate { Text(updates.statusTitle).font(.callout).foregroundStyle(Palette.accent) }
                        Spacer()
                        Text("协议核心 \(model.coreVersion.isEmpty ? "未加载" : model.coreVersion)").font(.caption).foregroundStyle(.secondary)
                    }
                }
            }
            Surface {
                VStack(alignment: .leading, spacing: 20) {
                    SectionHeading(title: "连接诊断", detail: "遇到问题时, 查看活动记录或打开日志目录.")
                    preference("详细日志", detail: "记录更多连接与生命周期细节", value: $preferences.verboseLogging)
                    HStack(spacing: 12) {
                        Button("活动与诊断") { model.page = .activity }.buttonStyle(ActionStyle())
                        Button("打开日志位置") { NSWorkspace.shared.activateFileViewerSelecting([AppPaths.logFile]) }
                        Button("蓝牙设置") { openBluetoothSettings() }
                    }
                }
            }
            if let error = preferences.persistenceError {
                Surface {
                    Label(error, systemImage: "exclamationmark.triangle.fill").foregroundStyle(.orange).font(.callout).textSelection(.enabled)
                }
            }
            HStack(alignment: .top, spacing: 12) {
                Image(systemName: "info.circle").foregroundStyle(.secondary)
                Text("菜单栏图标左键打开主窗口, 右键打开快捷菜单. 如需完全退出, 请在菜单栏或应用菜单中选择退出 EdifierCtrl.")
                    .font(.caption).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
            }.padding(.horizontal, 6)
        }
    }

    private func preference(_ title: String, detail: String, value: Binding<Bool>) -> some View {
        HStack {
            VStack(alignment: .leading, spacing: 6) {
                Text(title).font(.system(size: 14, weight: .medium))
                Text(detail).font(.caption).foregroundStyle(.secondary)
            }
            Spacer(minLength: 16)
            Toggle(title, isOn: value).labelsHidden().toggleStyle(.switch)
        }
    }
}

struct ActivityView: View {
    @ObservedObject var model: AppModel

    var body: some View {
        VStack(alignment: .leading, spacing: 22) {
            Surface {
                VStack(alignment: .leading, spacing: 18) {
                    HStack {
                        SectionHeading(title: "最近活动", detail: "保留本次运行的最近 200 条操作和连接事件.")
                        Spacer()
                        Button("清空") { model.clearActivity() }.disabled(model.activities.isEmpty)
                    }
                    if model.activities.isEmpty {
                        EmptyState(symbol: "waveform.path.ecg", title: "当前没有活动", detail: "连接耳机, 调整设置或发起交接后, 操作记录会出现在这里.")
                    } else {
                        LazyVStack(alignment: .leading, spacing: 0) {
                            ForEach(model.activities) { entry in
                                HStack(alignment: .top, spacing: 12) {
                                    Image(systemName: entry.isError ? "exclamationmark.circle.fill" : "circle.fill")
                                        .font(.system(size: entry.isError ? 12 : 6))
                                        .foregroundStyle(entry.isError ? Color.orange : Palette.mint).frame(width: 16, height: 18)
                                    VStack(alignment: .leading, spacing: 5) {
                                        Text(entry.title).font(.callout.weight(.medium))
                                        if !entry.detail.isEmpty { Text(entry.detail).font(.caption).foregroundStyle(.secondary).textSelection(.enabled) }
                                    }
                                    Spacer()
                                    Text(entry.date, style: .time).font(.system(size: 10, design: .monospaced)).foregroundStyle(.tertiary)
                                }.padding(.vertical, 12)
                                if entry.id != model.activities.last?.id { Divider().opacity(0.6) }
                            }
                        }
                    }
                }
            }
            DisclosureGroup("协议工具") {
                VStack(alignment: .leading, spacing: 14) {
                    Text("仅进行本地封装或解析, 不向耳机发送指令.").font(.caption).foregroundStyle(.secondary)
                    TextEditor(text: $model.diagnosticInput).font(.system(.body, design: .monospaced))
                        .frame(height: 80).padding(8).background(Palette.canvas, in: RoundedRectangle(cornerRadius: 10))
                        .accessibilityLabel("待解析的 JSON 命令或十六进制帧")
                    HStack {
                        Button("封装 JSON 命令") { model.inspect(frame: false) }
                        Button("解析十六进制帧") { model.inspect(frame: true) }
                    }.disabled(!model.ready || model.busy)
                    if !model.diagnosticOutput.isEmpty {
                        Text(model.diagnosticOutput).font(.system(size: 12, design: .monospaced)).textSelection(.enabled)
                            .frame(maxWidth: .infinity, alignment: .leading).padding(14)
                            .background(Palette.canvas, in: RoundedRectangle(cornerRadius: 10))
                    }
                }
            }
        }
    }
}
