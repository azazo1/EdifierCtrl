import SwiftUI

struct HeadphonesView: View {
    @ObservedObject var model: AppModel
    @ObservedObject private var preferences = AppPreferences.shared
    @State private var danger: HeadphoneAction?

    var body: some View {
        VStack(alignment: .leading, spacing: 22) {
            hero
            if model.isConnected {
                if model.selectedProfile?.supports("noise") == true { noiseCard }
                HStack(alignment: .top, spacing: 18) {
                    Surface { playback }
                    Surface {
                        VStack(alignment: .leading, spacing: 16) {
                            SectionHeading(title: "此刻的连接", detail: model.audioLabel)
                            Metric(label: "控制通道", value: "经典蓝牙 · 已连接", symbol: "antenna.radiowaves.left.and.right")
                            Metric(label: "跨设备交接", value: model.groupJoined ? "组内 \(model.peers.count + 1) 台设备" : "尚未加入交接组", symbol: "arrow.triangle.swap")
                            Button("前往交接") { model.page = .handoff }.buttonStyle(ActionStyle())
                        }
                    }
                }
                advancedControls
                Surface {
                    VStack(alignment: .leading, spacing: 18) {
                        SectionHeading(title: "设备信息")
                        HStack(alignment: .top, spacing: 28) {
                            Metric(label: "固件版本", value: model.state.firmware ?? "等待耳机回报", symbol: "cpu")
                            Metric(label: "蓝牙地址", value: model.state.mac ?? model.state.address ?? "未读取", symbol: "number")
                        }
                        Divider()
                        HStack {
                            Text("机型档案").font(.callout)
                            Picker("机型档案", selection: Binding(get: { preferences.selectedProfile }, set: { model.selectProfile($0) })) {
                                ForEach(model.profiles) { Text($0.displayName).tag($0.id) }
                            }.labelsHidden().frame(maxWidth: 230).disabled(model.busy)
                            Spacer()
                            Button("刷新状态") { model.refreshReadings() }.disabled(!model.canControl)
                        }
                        Text("按机型显示支持的功能. 通用档案中的部分功能可能不被你的耳机支持.")
                            .font(.caption).foregroundStyle(.secondary)
                    }
                }
                Surface {
                    DisclosureGroup("设备维护") {
                        VStack(alignment: .leading, spacing: 16) {
                            Text("这些操作会中断当前连接. 恢复出厂还会清除耳机配对记录.").font(.caption).foregroundStyle(.secondary)
                            HStack(spacing: 10) {
                                ForEach(HeadphoneAction.allCases) { action in
                                    Button(action.title) { danger = action }.disabled(!model.canControl)
                                }
                            }
                        }.padding(.top, 14)
                    }
                }
            } else {
                Surface {
                    VStack(spacing: 18) {
                        EmptyState(symbol: "antenna.radiowaves.left.and.right", title: "准备好, 开始聆听", detail: "先在 macOS 蓝牙设置中完成配对, 再选择你的耳机. 连接后可调整降噪, 音效与更多选项.")
                        HStack(spacing: 12) {
                            Button("连接耳机") { model.page = .devices; model.scan() }.buttonStyle(ActionStyle(prominent: true)).disabled(!model.ready || model.busy)
                            if !preferences.lastDeviceAddress.isEmpty {
                                Button("连接上次的耳机") { model.reconnect() }.buttonStyle(ActionStyle()).disabled(!model.ready || model.busy)
                            }
                        }.padding(.bottom, 12)
                    }.frame(maxWidth: .infinity)
                }
            }
        }
        .confirmationDialog(danger?.title ?? "设备维护", isPresented: Binding(get: { danger != nil }, set: { if !$0 { danger = nil } }), titleVisibility: .visible) {
            if let danger {
                Button(danger.title, role: .destructive) { model.send(danger.rawValue, label: danger.title) }
            }
            Button("取消", role: .cancel) { danger = nil }
        } message: { Text(danger?.detail ?? "") }
    }

    private var hero: some View {
        HStack(spacing: 28) {
            ZStack {
                Circle().fill(Palette.accent.opacity(0.045)).frame(width: 186, height: 186)
                Circle().stroke(Palette.accent.opacity(0.10), lineWidth: 1).frame(width: 152, height: 152)
                Image(systemName: "headphones").font(.system(size: 94, weight: .ultraLight))
                    .foregroundStyle(LinearGradient(colors: [Palette.accent, Palette.accent.opacity(0.55)], startPoint: .topLeading, endPoint: .bottomTrailing))
                    .shadow(color: Palette.accent.opacity(0.14), radius: 12, x: 0, y: 10)
            }.accessibilityHidden(true)
            VStack(alignment: .leading, spacing: 14) {
                Text("EDIFIER / PERSONAL AUDIO").font(.system(size: 10, weight: .semibold, design: .monospaced)).tracking(2).foregroundStyle(.secondary)
                Text(model.headphoneName).font(.system(size: 29, weight: .bold, design: .rounded)).lineLimit(2)
                HStack(spacing: 10) {
                    StatusPill(text: model.isConnected ? "控制已连接" : "等待连接", color: model.isConnected ? Palette.mint : .secondary)
                    if let battery = model.state.battery {
                        Label("\(battery)%", systemImage: battery > 20 ? "battery.75percent" : "battery.25percent")
                            .font(.callout.weight(.medium)).foregroundStyle(battery > 20 ? Palette.mint : .orange)
                    }
                }
                if model.isConnected {
                    HStack(spacing: 12) {
                        Button("读取状态") { model.refreshReadings() }.buttonStyle(ActionStyle()).disabled(!model.canControl)
                        Button("断开控制") { model.disconnect() }.buttonStyle(.plain).font(.callout).foregroundStyle(.secondary).disabled(model.busy)
                    }.padding(.top, 4)
                } else {
                    Text("控制音效, 管理连接, 在设备之间自由切换.").font(.callout).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
                }
            }
            Spacer(minLength: 0)
        }
        .padding(24).frame(maxWidth: .infinity, alignment: .leading)
        .background(LinearGradient(colors: [Palette.accent.opacity(0.08), Palette.card], startPoint: .topLeading, endPoint: .bottomTrailing), in: RoundedRectangle(cornerRadius: 24))
        .overlay(RoundedRectangle(cornerRadius: 24).stroke(Palette.accent.opacity(0.08), lineWidth: 1))
    }

    private var noiseCard: some View {
        Surface {
            VStack(alignment: .leading, spacing: 20) {
                HStack {
                    SectionHeading(title: "聆听模式", detail: "隔绝喧闹, 或留意身边的声音.")
                    Spacer()
                    if model.state.noise == nil { Text("等待状态回报").font(.caption).foregroundStyle(.secondary) }
                }
                HStack(spacing: 12) {
                    noiseChoice("normal", title: "标准", subtitle: "自然聆听", symbol: "waveform")
                    noiseChoice("reduction", title: "主动降噪", subtitle: "专注眼前", symbol: "waveform.slash")
                    if model.selectedProfile?.supports("ambient_sound") == true {
                        noiseChoice("ambient", title: "环境声", subtitle: "听见周围", symbol: "ear.badge.waveform")
                    }
                }
                if model.state.noise == "ambient", model.selectedProfile?.supports("ambient_sound") == true {
                    Divider()
                    LevelControl(title: "环境声强度", detail: "仅在环境声模式下生效", value: model.state.ambientVolume, range: -3...3, enabled: model.canControl) {
                        model.send("set_ambient_volume", values: ["volume": $0], label: "设置环境声强度", query: "query_noise")
                    }
                }
            }
        }
    }

    private func noiseChoice(_ mode: String, title: String, subtitle: String, symbol: String) -> some View {
        ChoiceTile(title: title, subtitle: subtitle, symbol: symbol, selected: model.state.noise == mode) {
            model.send("set_noise_mode", values: ["mode": mode], label: "切换\(title)", query: "query_noise")
        }.disabled(!model.canControl)
    }

    private var playback: some View {
        VStack(alignment: .leading, spacing: 18) {
            SectionHeading(title: "播放控制", detail: "控制耳机正在播放的音频")
            HStack(spacing: 12) {
                playbackButton("previous", symbol: "backward.end.fill", label: "上一首")
                playbackButton("play", symbol: "play.fill", label: "播放")
                playbackButton("pause", symbol: "pause.fill", label: "暂停")
                playbackButton("next", symbol: "forward.end.fill", label: "下一首")
            }
            HStack(spacing: 12) {
                playbackButton("volume_down", symbol: "speaker.minus.fill", label: "降低音量")
                Text("耳机音量").font(.caption).foregroundStyle(.secondary)
                playbackButton("volume_up", symbol: "speaker.plus.fill", label: "提高音量")
            }
        }.disabled(!model.canControl || model.selectedProfile?.supports("playback") != true)
    }

    private func playbackButton(_ action: String, symbol: String, label: String) -> some View {
        Button { model.send("playback", values: ["action": action], label: label) } label: {
            Image(systemName: symbol).font(.system(size: 14)).frame(width: 40, height: 38)
                .background(Palette.canvas, in: RoundedRectangle(cornerRadius: 10))
        }.buttonStyle(.plain).help(label).accessibilityLabel(label)
    }

    private var advancedControls: some View {
        HeadphoneSettingsView(model: model)
    }
}

enum HeadphoneAction: String, CaseIterable, Identifiable {
    case disconnectHost = "disconnect_host", powerOff = "power_off", rePair = "re_pair", factoryReset = "factory_reset"
    var id: String { rawValue }
    var title: String {
        switch self {
        case .disconnectHost: return "释放当前主机"
        case .powerOff: return "关闭耳机"
        case .rePair: return "重新配对"
        case .factoryReset: return "恢复出厂"
        }
    }
    var detail: String {
        switch self {
        case .disconnectHost: return "耳机将断开当前主机的蓝牙连接. 日常切换设备请优先使用交接功能."
        case .powerOff: return "耳机将关机, 当前播放会立即停止."
        case .rePair: return "耳机将进入配对模式并中断当前连接."
        case .factoryReset: return "这会清除耳机设置和配对记录, 之后需要在各台设备上重新配对."
        }
    }
}
