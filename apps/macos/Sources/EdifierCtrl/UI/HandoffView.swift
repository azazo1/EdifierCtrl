import SwiftUI

struct HandoffView: View {
    @ObservedObject var model: AppModel
    @ObservedObject private var preferences = AppPreferences.shared
    @State private var manualAddress = ""

    var body: some View {
        VStack(alignment: .leading, spacing: 22) {
            intro
            if let progress = model.state.handoff { progressCard(progress) }
            if model.groupJoined {
                localDevice
                HStack {
                    SectionHeading(title: "附近的设备", detail: "点击接管, 将对方正在使用的耳机交给这台 Mac.")
                    Spacer()
                    StatusPill(text: "自动发现中", color: Palette.accent)
                }
                if model.peers.isEmpty {
                    Surface {
                        EmptyState(symbol: "laptopcomputer.and.iphone", title: "等待你的其他设备", detail: "在 Android, Windows 或 Linux 客户端使用相同组名加入交接组. 确保设备处于同一局域网, 并允许应用访问本地网络.")
                    }
                } else {
                    ForEach(model.peers) { peer in peerRow(peer) }
                }
                DisclosureGroup("通过耳机地址接管") {
                    VStack(alignment: .leading, spacing: 12) {
                        Text("耳机必须已在这台 Mac 上配对. 确保原设备也已加入同一组.")
                            .font(.caption).foregroundStyle(.secondary)
                        HStack {
                            TextField("AA:BB:CC:DD:EE:FF", text: $manualAddress).textFieldStyle(.roundedBorder)
                            Button("接管到这台 Mac") { model.claim(address: manualAddress) }
                                .disabled(model.busy || BluetoothAddress.normalize(manualAddress) == nil)
                        }
                    }
                }
            }
            Surface {
                VStack(alignment: .leading, spacing: 14) {
                    SectionHeading(title: "交接前, 准备这三件事")
                    helpRow("1", title: "分别完成配对", detail: "让耳机先与需要使用的手机和电脑分别配对一次.")
                    helpRow("2", title: "使用同一局域网", detail: "各端打开 EdifierCtrl, 输入完全相同的组名.")
                    helpRow("3", title: "在目标设备接管", detail: "原设备释放音频后, 目标设备尝试连接耳机. 蓝牙与系统权限会影响接管结果.")
                }
            }
        }
    }

    private var intro: some View {
        Surface {
            VStack(alignment: .leading, spacing: 22) {
                HStack(spacing: 18) {
                    Image(systemName: "laptopcomputer.and.iphone").font(.system(size: 36, weight: .light)).foregroundStyle(Palette.accent)
                        .frame(width: 74, height: 66).background(Palette.accent.opacity(0.08), in: RoundedRectangle(cornerRadius: 18))
                    VStack(alignment: .leading, spacing: 7) {
                        Text(model.groupJoined ? "设备已在同一交接组" : "把你的设备连在一起").font(.system(size: 19, weight: .semibold))
                        Text("Android · macOS · Windows · Linux").font(.callout).foregroundStyle(.secondary)
                    }
                    Spacer()
                }
                Divider()
                if model.groupJoined {
                    HStack {
                        VStack(alignment: .leading, spacing: 6) {
                            StatusPill(text: "已加入交接组")
                            Text("组名: \(model.joinedGroupName)")
                                .font(.callout.weight(.semibold)).textSelection(.enabled)
                                .fixedSize(horizontal: false, vertical: true)
                            Text("关闭窗口后仍可响应其他设备的交接请求.").font(.caption).foregroundStyle(.secondary)
                        }
                        Spacer()
                        Button("退出交接组") { model.leaveGroup() }.disabled(model.operation != nil)
                    }
                } else {
                    HStack(spacing: 12) {
                        TextField("输入与其他设备相同的组名", text: $model.groupSecret)
                            .textFieldStyle(.roundedBorder).onSubmit { model.joinGroup() }
                        Button("加入交接组") { model.joinGroup() }.buttonStyle(ActionStyle(prominent: true))
                            .disabled(!model.ready || model.busy || model.groupSecret.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                    }
                    HStack {
                        Toggle("记住组名", isOn: $model.rememberGroup).toggleStyle(.checkbox).font(.callout)
                        Spacer()
                        Button("读取已保存组名") { model.loadRememberedGroup() }.buttonStyle(.link)
                    }
                    Toggle("启动时自动加入", isOn: $preferences.autoJoinGroup).toggleStyle(.checkbox).font(.callout)
                        .disabled(!model.rememberGroup)
                }
            }
        }
    }

    private var localDevice: some View {
        Surface(padding: 18) {
            HStack(spacing: 16) {
                Image(systemName: "laptopcomputer").font(.system(size: 30, weight: .light)).foregroundStyle(Palette.accent)
                    .frame(width: 58, height: 52)
                VStack(alignment: .leading, spacing: 6) {
                    HStack {
                        Text("这台 Mac").font(.headline)
                        Text("本机").font(.caption2.weight(.medium)).padding(.horizontal, 7).padding(.vertical, 3)
                            .background(Palette.accent.opacity(0.1), in: Capsule()).foregroundStyle(Palette.accent)
                    }
                    Text(model.state.holding.map { "正在持有 \($0)" } ?? "尚未确认持有耳机音频")
                        .font(.caption).foregroundStyle(.secondary).textSelection(.enabled)
                }
                Spacer()
                Text(model.audioLabel).font(.caption).foregroundStyle(.secondary)
            }
        }
    }

    private func peerRow(_ peer: GroupPeer) -> some View {
        Surface(padding: 20) {
            HStack(spacing: 16) {
                Image(systemName: peer.symbol).font(.system(size: 28, weight: .light)).foregroundStyle(Palette.accent)
                    .frame(width: 58, height: 58).background(Palette.accent.opacity(0.06), in: RoundedRectangle(cornerRadius: 16))
                VStack(alignment: .leading, spacing: 7) {
                    Text(peer.displayName).font(.system(size: 15, weight: .semibold))
                    Text("\(peer.platformName) · \(peer.appVersion)").font(.caption).foregroundStyle(.secondary)
                    if let holding = peer.holding, !holding.isEmpty {
                        Label(holding, systemImage: "headphones").font(.system(size: 11, design: .monospaced)).foregroundStyle(Palette.mint)
                    } else { Text("当前没有持有耳机").font(.caption).foregroundStyle(.secondary) }
                }
                Spacer()
                Button("接管到这台 Mac") { model.claim(peer) }
                    .buttonStyle(ActionStyle(prominent: true))
                    .disabled(model.busy || !peer.canAudio || peer.holding?.isEmpty != false || model.state.holding == peer.holding)
                    .help(peer.canAudio ? "请求对方释放耳机, 随后连接本机音频" : "该成员不支持音频交接")
            }
        }
    }

    private func progressCard(_ progress: HandoffProgress) -> some View {
        Surface {
            VStack(alignment: .leading, spacing: 20) {
                HStack(spacing: 12) {
                    if progress.isActive { ProgressView().controlSize(.small) }
                    else { Image(systemName: progress.kind == "done" ? "checkmark.circle.fill" : "exclamationmark.circle.fill").foregroundStyle(progress.kind == "done" ? Palette.mint : Color.orange) }
                    SectionHeading(title: progress.title, detail: progress.reason)
                }
                HStack(spacing: 10) {
                    ForEach(Array(["发起请求", "释放原连接", "建立新连接", "完成"].enumerated()), id: \.offset) { index, title in
                        VStack(alignment: .leading, spacing: 9) {
                            Capsule().fill(index <= progress.step ? Palette.accent : Color.primary.opacity(0.08)).frame(height: 4)
                            Text(title).font(.caption).foregroundStyle(index <= progress.step ? Color.primary : Color.secondary)
                        }
                    }
                }
                if progress.kind == "failed" {
                    Text("确认耳机已配对并开启, 原设备客户端保持运行. 若音频未自动接通, 可从 macOS 蓝牙设置连接耳机后重试.")
                        .font(.caption).foregroundStyle(.secondary)
                    Button("打开蓝牙设置") { openBluetoothSettings() }.buttonStyle(.link)
                }
            }
        }
    }

    private func helpRow(_ number: String, title: String, detail: String) -> some View {
        HStack(alignment: .top, spacing: 13) {
            Text(number).font(.system(size: 11, weight: .semibold, design: .rounded)).frame(width: 24, height: 24)
                .background(Palette.accent.opacity(0.08), in: Circle()).foregroundStyle(Palette.accent)
            VStack(alignment: .leading, spacing: 4) {
                Text(title).font(.callout.weight(.medium))
                Text(detail).font(.caption).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
            }
        }
    }
}
