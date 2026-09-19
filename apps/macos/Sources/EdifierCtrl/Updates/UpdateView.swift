import SwiftUI

struct UpdateView: View {
    @ObservedObject private var manager = UpdateManager.shared
    @ObservedObject private var preferences = AppPreferences.shared
    @State private var confirmInstall = false

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            HStack {
                Text("软件更新").font(.title2.bold())
                Spacer()
                if manager.phase != .handedOff {
                    Button("关闭") { manager.isPresented = false }
                        .keyboardShortcut(.cancelAction)
                }
            }
            Text("当前版本: \(AppVersion.display)").foregroundStyle(.secondary)
            status
            if let candidate = manager.candidate {
                ScrollView {
                    Text(candidate.release.body?.isEmpty == false ? candidate.release.body! : "此版本没有提供更新说明.")
                        .textSelection(.enabled)
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
                .frame(minHeight: 100, maxHeight: 220)
                if manager.phase != .handedOff {
                    Link("查看 Release 页", destination: candidate.release.htmlURL)
                }
            }
            if manager.phase == .downloading {
                ProgressView(value: Double(manager.received), total: Double(max(1, manager.total)))
                Text("\(ByteCountFormatter.string(fromByteCount: manager.received, countStyle: .file)) / \(ByteCountFormatter.string(fromByteCount: manager.total, countStyle: .file))")
                    .font(.caption).monospacedDigit()
                Text("关闭此窗口后下载仍会继续.").font(.caption).foregroundStyle(.secondary)
            }
            if manager.phase != .handedOff {
                Toggle("启动时自动检查更新", isOn: $preferences.autoCheckUpdates)
                if !manager.notificationsAuthorized && Bundle.main.bundleURL.pathExtension == "app" {
                    Button("允许更新通知") { manager.requestNotificationPermission() }
                        .font(.caption)
                }
                HStack {
                    Button("重新检查") { manager.checkForUpdates(userInitiated: true) }
                        .disabled(manager.isBusy)
                    Spacer()
                    actions
                }
            }
        }
        .padding(24)
        .frame(width: 540)
        .interactiveDismissDisabled(manager.phase == .handedOff)
        .task { await manager.refreshNotificationPermission() }
        .confirmationDialog("重启并安装已验证的更新?", isPresented: $confirmInstall, titleVisibility: .visible) {
            Button("重启并更新") { manager.restartAndInstall() }
            Button("取消", role: .cancel) {}
        } message: {
            Text("应用将停止后台服务并退出, 由更新助手替换应用后重新打开. 便携运行时会打开 DMG 供手动安装.")
        }
    }

    @ViewBuilder private var status: some View {
        switch manager.phase {
        case .idle:
            Text(AppVersion.display == "dev-build" ? "开发构建不检查发行版更新." : "可以检查最新发行版.")
        case .checking:
            ProgressView("正在检查更新...")
        case .upToDate:
            Label("当前已是最新版本", systemImage: "checkmark.circle")
        case .available:
            Text(manager.statusTitle).bold()
        case .downloading:
            Text("正在后台下载并校验更新...")
        case .readyToRestart:
            Text("新版本已下载, 重启后生效.").bold()
            Text("当前版本可继续正常使用. 由你决定何时重启并更新.").foregroundStyle(.secondary)
        case .handedOff:
            ProgressView("正在退出并替换, 请勿手动结束进程...")
        case .dmgOpened:
            Text("已打开安装包. 请退出应用后, 将 EdifierCtrl 拖入 Applications 完成安装.")
        case .failed(let message):
            Label("更新未完成", systemImage: "exclamationmark.triangle").foregroundStyle(.orange)
            Text(message).textSelection(.enabled)
        }
    }

    @ViewBuilder private var actions: some View {
        switch manager.phase {
        case .available:
            Button("跳过此版本") { manager.skipVersion() }
            Button("立即更新") { manager.downloadUpdate() }.buttonStyle(.borderedProminent)
        case .downloading:
            Button("取消更新") { manager.cancelDownload() }
        case .readyToRestart:
            Button("重启并更新") { confirmInstall = true }.buttonStyle(.borderedProminent)
        case .failed:
            if manager.candidate != nil && manager.installationFailure == nil {
                Button("重试下载") { manager.downloadUpdate() }
            }
        default:
            EmptyView()
        }
    }
}
