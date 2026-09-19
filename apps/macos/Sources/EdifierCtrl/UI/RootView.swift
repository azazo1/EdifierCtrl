import AppKit
import SwiftUI

struct RootView: View {
    @ObservedObject var model: AppModel
    @ObservedObject private var updates = UpdateManager.shared
    @ObservedObject private var preferences = AppPreferences.shared

    var body: some View {
        WorkspaceLayout {
            sidebar
        } content: {
            VStack(spacing: 0) {
                header
                Divider().opacity(0.5)
                ScrollView {
                    Group {
                        switch model.page {
                        case .headphones: HeadphonesView(model: model)
                        case .devices: DevicesView(model: model)
                        case .handoff: HandoffView(model: model)
                        case .settings: PreferencesView(model: model)
                        case .activity: ActivityView(model: model)
                        }
                    }
                    .padding(28).frame(maxWidth: 1100).frame(maxWidth: .infinity)
                }
                .background(Palette.canvas)
                if let notice = model.notice { noticeBar(notice) }
            }
        }
        .tint(Palette.accent)
        .disclosureGroupStyle(CardDisclosureStyle())
        .sheet(isPresented: $updates.isPresented) { UpdateView() }
    }

    private var sidebar: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 11) {
                Image(systemName: "waveform").font(.system(size: 22, weight: .semibold))
                    .foregroundStyle(.white).frame(width: 42, height: 42)
                    .background(Palette.accent.gradient, in: RoundedRectangle(cornerRadius: 13))
                VStack(alignment: .leading, spacing: 3) {
                    Text("EdifierCtrl").font(.system(size: 17, weight: .bold, design: .rounded))
                    Text("声音, 随你而行").font(.system(size: 10)).foregroundStyle(.secondary)
                }
            }
            .padding(.horizontal, 20).padding(.top, 28).padding(.bottom, 34)
            Text("工作台").font(.system(size: 10, weight: .semibold)).foregroundStyle(.tertiary)
                .padding(.horizontal, 24).padding(.bottom, 10)
            ForEach([AppPage.headphones, .devices, .handoff]) { navigationItem($0) }
            Spacer(minLength: 24)
            ForEach([AppPage.settings, .activity]) { navigationItem($0) }
            Divider().padding(.horizontal, 20).padding(.vertical, 18)
            VStack(alignment: .leading, spacing: 12) {
                HStack(spacing: 8) {
                    Circle().fill(model.ready ? Palette.mint : Color.orange).frame(width: 6, height: 6)
                    Text(model.ready ? "后台服务运行中" : "耳机服务未就绪").font(.caption).foregroundStyle(.secondary)
                }
                Button {
                    updates.isPresented = true
                } label: {
                    HStack {
                        Text(updates.hasUpdate ? updates.statusTitle : AppVersion.display)
                            .lineLimit(1).font(.system(size: 11, design: .monospaced))
                        Spacer()
                        Image(systemName: updates.hasUpdate ? "arrow.down.circle.fill" : "info.circle").font(.caption)
                    }
                    .frame(maxWidth: .infinity, minHeight: 28)
                    .contentShape(Rectangle())
                }
                .buttonStyle(QuietButtonStyle(cornerRadius: 6)).foregroundStyle(updates.hasUpdate ? Palette.accent : Color.secondary)
                .help("版本与更新")
            }
            .padding(.horizontal, 24).padding(.bottom, 24)
        }
        .frame(width: 212)
    }

    private func navigationItem(_ page: AppPage) -> some View {
        Button { model.page = page } label: {
            HStack(spacing: 12) {
                Image(systemName: page.symbol).font(.system(size: 15, weight: .medium)).frame(width: 20)
                Text(page.title).font(.system(size: 13, weight: model.page == page ? .semibold : .regular))
                Spacer()
                if page == .handoff && model.groupJoined {
                    Text("\(model.peers.count)").font(.caption2.weight(.semibold)).padding(.horizontal, 6).padding(.vertical, 2)
                        .background(Palette.accent.opacity(0.1), in: Capsule())
                }
            }
            .foregroundStyle(model.page == page ? Palette.accent : Color.primary.opacity(0.72))
            .padding(.horizontal, 13).padding(.vertical, 13)
            .contentShape(Rectangle())
        }
        .buttonStyle(QuietButtonStyle(selected: model.page == page)).padding(.horizontal, 12).padding(.vertical, 2)
        .accessibilityAddTraits(model.page == page ? .isSelected : [])
    }

    private var header: some View {
        HStack(alignment: .center) {
            VStack(alignment: .leading, spacing: 6) {
                Text(model.page.title).font(.system(size: 25, weight: .bold))
                Text(model.page.subtitle).font(.callout).foregroundStyle(.secondary)
            }
            Spacer()
            if let operation = model.operation {
                ProgressView().controlSize(.small)
                Text(operation).font(.caption).foregroundStyle(.secondary)
            } else {
                StatusPill(text: model.isConnected ? "耳机已连接" : "未连接耳机", color: model.isConnected ? Palette.mint : .secondary)
            }
        }
        .padding(.horizontal, 30).padding(.vertical, 24)
    }

    private func noticeBar(_ notice: UserNotice) -> some View {
        HStack(alignment: .top, spacing: 12) {
            Image(systemName: notice.isError ? "exclamationmark.circle.fill" : "checkmark.circle.fill")
                .foregroundStyle(notice.isError ? Color.orange : Palette.mint).padding(.top, 2)
            VStack(alignment: .leading, spacing: 4) {
                Text(notice.title).font(.callout.weight(.semibold))
                Text(notice.detail).font(.caption).foregroundStyle(.secondary).textSelection(.enabled)
            }
            Spacer()
            Button { model.notice = nil } label: { Image(systemName: "xmark").font(.caption) }
                .buttonStyle(IconButtonStyle(width: 30, height: 30, cornerRadius: 8)).help("关闭提示").accessibilityLabel("关闭提示")
        }
        .padding(16).background(.regularMaterial)
        .overlay(alignment: .top) { Divider() }
    }
}
