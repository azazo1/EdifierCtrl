import SwiftUI

struct HeadphoneSettingsView: View {
    @ObservedObject var model: AppModel
    @State private var nameDraft = ""
    @State private var normal = true
    @State private var reduction = true
    @State private var ambient = true
    @State private var controlsDirty = false
    @State private var pendingLDAC: String?

    var body: some View {
        VStack(alignment: .leading, spacing: 20) {
            if supports("sound_effect") {
                Surface {
                    VStack(alignment: .leading, spacing: 18) {
                        SectionHeading(title: "声音风格", detail: "找到适合当前音乐的音效.")
                        HStack(spacing: 10) {
                            effect("normal", title: "标准", symbol: "slider.horizontal.3")
                            effect("pop", title: "流行", symbol: "music.note")
                            effect("classical", title: "古典", symbol: "pianokeys")
                            effect("rock", title: "摇滚", symbol: "guitars")
                        }.disabled(!model.canControl)
                    }
                }
            }
            DisclosureGroup {
                VStack(alignment: .leading, spacing: 22) {
                    if supports("game_mode") {
                        settingToggle("低延迟游戏模式", detail: "适合游戏与视频通话", value: model.state.gameMode) {
                            model.send("set_game_mode", values: ["on": $0], label: "设置游戏模式", query: "query_game_mode")
                        }
                    }
                    if supports("auto_power_off") {
                        settingToggle("空闲自动关机", detail: "由耳机管理待机耗电", value: model.state.autoPowerOff) {
                            model.send("set_auto_power_off", values: ["on": $0], label: "设置自动关机", query: "query_auto_power_off")
                        }
                    }
                    if supports("prompt_volume") {
                        Divider()
                        LevelControl(title: "提示音量", detail: "耳机开关机与模式切换提示", value: model.state.promptVolume, range: 0...15, enabled: model.canControl) {
                            model.send("set_prompt_volume", values: ["volume": $0], label: "设置提示音量", query: "query_prompt_volume")
                        }
                    }
                    if supports("shutdown_timer") { timerSettings }
                    if supports("control_settings") { buttonSettings }
                    if supports("ldac") { ldacSettings }
                    if supports("name") { nameSettings }
                }
            } label: {
                SectionHeading(title: "更多耳机设置", detail: "提示音, 定时关机与按键偏好")
            }
        }
        .onAppear { synchronizeControls() }
        .onChange(of: model.state.controlNormal) { _ in if !controlsDirty { synchronizeControls() } }
        .onChange(of: model.state.controlReduction) { _ in if !controlsDirty { synchronizeControls() } }
        .onChange(of: model.state.controlAmbient) { _ in if !controlsDirty { synchronizeControls() } }
        .confirmationDialog("更改 LDAC 设置", isPresented: Binding(get: { pendingLDAC != nil }, set: { if !$0 { pendingLDAC = nil } }), titleVisibility: .visible) {
            if let mode = pendingLDAC {
                Button("更改设置") { model.send("set_ldac", values: ["mode": mode], label: "设置 LDAC", query: "query_ldac") }
            }
            Button("取消", role: .cancel) { pendingLDAC = nil }
        } message: { Text("此项修改耳机的编码设置, 可能导致蓝牙重新连接. macOS 本身不提供 LDAC 音频输出.") }
    }

    private func supports(_ feature: String) -> Bool { model.selectedProfile?.supports(feature) == true }

    private func effect(_ value: String, title: String, symbol: String) -> some View {
        Button { model.send("set_sound_effect", values: ["effect": value], label: "切换\(title)音效", query: "query_sound_effect") } label: {
            VStack(spacing: 10) {
                Image(systemName: symbol).font(.title3)
                Text(title).font(.callout.weight(.medium))
            }
            .frame(maxWidth: .infinity).padding(.vertical, 18)
            .foregroundStyle(model.state.effect == value ? Palette.accent : Color.secondary)
        }.buttonStyle(ChoiceButtonStyle(selected: model.state.effect == value, cornerRadius: 12))
            .accessibilityValue(model.state.effect == value ? "已选中" : "未选中")
    }

    private func settingToggle(_ title: String, detail: String, value: Bool?, action: @escaping (Bool) -> Void) -> some View {
        HStack {
            SectionHeading(title: title, detail: detail)
            Spacer()
            if value == nil { Text("未读取").font(.caption).foregroundStyle(.secondary) }
            Toggle(title, isOn: Binding(get: { value ?? false }, set: action)).labelsHidden().toggleStyle(.switch).disabled(!model.canControl)
        }
    }

    private var timerSettings: some View {
        VStack(alignment: .leading, spacing: 18) {
            Divider()
            HStack {
                SectionHeading(title: "定时关机", detail: "设置仅在本次开机期间有效")
                Spacer()
                Text(model.state.shutdownEnabled == true ? "已开启" : model.state.shutdownEnabled == false ? "未开启" : "未读取")
                    .font(.caption).foregroundStyle(.secondary)
                Button("关闭定时") { model.send("disable_shutdown_timer", label: "关闭定时关机", query: "query_shutdown_timer") }
                    .disabled(!model.canControl || model.state.shutdownEnabled != true)
            }
            LevelControl(title: "关机倒计时", detail: "点击应用后开启定时", value: model.state.shutdownMinutes, range: 1...180, unit: " 分钟", enabled: model.canControl) {
                model.send("set_shutdown_timer", values: ["minutes": $0], label: "设置定时关机", query: "query_shutdown_timer")
            }
        }
    }

    private var buttonSettings: some View {
        VStack(alignment: .leading, spacing: 16) {
            Divider()
            SectionHeading(title: "耳机按键切换模式", detail: "至少选择两种模式, 按耳机按键时依次切换.")
            HStack(spacing: 18) {
                Toggle("标准", isOn: $normal).onChange(of: normal) { _ in controlsDirty = true }
                Toggle("降噪", isOn: $reduction).onChange(of: reduction) { _ in controlsDirty = true }
                Toggle("环境声", isOn: $ambient).onChange(of: ambient) { _ in controlsDirty = true }
                Spacer()
                Button("恢复") { synchronizeControls() }.disabled(!controlsDirty)
                Button("应用") {
                    model.send("set_control_settings", values: ["normal": normal, "reduction": reduction, "ambient": ambient], label: "设置按键模式", query: "query_control_settings")
                    controlsDirty = false
                }.disabled(!model.canControl || !controlsDirty || [normal, reduction, ambient].filter { $0 }.count < 2)
            }.toggleStyle(.checkbox)
        }
    }

    private var ldacSettings: some View {
        VStack(alignment: .leading, spacing: 14) {
            Divider()
            SectionHeading(title: "LDAC 编码偏好", detail: "更改耳机的设置, 供支持 LDAC 的设备使用. macOS 不支持 LDAC 输出.")
            Picker("LDAC 编码偏好", selection: Binding(get: { model.state.ldac ?? "unknown" }, set: { pendingLDAC = $0 })) {
                Text("未读取").tag("unknown")
                Text("关闭").tag("off")
                Text("44.1 / 48 kHz").tag("rate48k")
                Text("96 kHz").tag("rate96k")
            }.labelsHidden().frame(maxWidth: 260).disabled(!model.canControl)
        }
    }

    private var nameSettings: some View {
        VStack(alignment: .leading, spacing: 14) {
            Divider()
            SectionHeading(title: "耳机名称", detail: "新名称可能需要移除系统配对记录并重新配对后显示.")
            HStack {
                TextField(model.headphoneName, text: $nameDraft).textFieldStyle(.roundedBorder)
                Button("应用名称") { model.send("set_name", values: ["name": nameDraft], label: "更新耳机名称", query: "query_name") }
                    .disabled(!model.canControl || nameDraft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
            }
        }
    }

    private func synchronizeControls() {
        normal = model.state.controlNormal ?? true
        reduction = model.state.controlReduction ?? true
        ambient = model.state.controlAmbient ?? true
        controlsDirty = false
    }
}
