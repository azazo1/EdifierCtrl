import AppKit
import Combine
import Foundation

@MainActor
final class AppModel: ObservableObject {
    @Published var page: AppPage = .headphones
    @Published private(set) var state = SessionState()
    @Published private(set) var devices: [HeadphoneDevice] = []
    @Published private(set) var profiles: [HeadphoneProfile] = []
    @Published private(set) var peers: [GroupPeer] = []
    @Published private(set) var activities: [ActivityEntry] = []
    @Published private(set) var isConnected = false
    @Published private(set) var connectionLabel = "尚未连接耳机"
    @Published private(set) var ready = false
    @Published private(set) var operation: String?
    @Published private(set) var scanning = false
    @Published private(set) var groupJoined = false
    @Published private(set) var coreVersion = ""
    @Published var notice: UserNotice?
    @Published var groupSecret = ""
    @Published var rememberGroup = true
    @Published var diagnosticInput = "{\"op\":\"query_battery\"}"
    @Published private(set) var diagnosticOutput = ""

    private let native: EdifierNative
    private var pollTask: Task<Void, Never>?
    private var startupTask: Task<Void, Never>?
    private var operationTask: Task<Void, Never>?
    private var handoffStarted: Date?
    private var claimTarget: String?
    private var shuttingDown = false
    private var hasStarted = false
    private var hasReportedPollFailure = false

    init() {
        let name = Host.current().localizedName ?? "Mac"
        native = EdifierNative(localID: "\(name)-\(AppPreferences.shared.installationID)")
    }

    var busy: Bool { operation != nil || state.handoff?.isActive == true }
    var canControl: Bool { ready && isConnected && !busy }
    var selectedProfile: HeadphoneProfile? {
        profiles.first { $0.id == AppPreferences.shared.selectedProfile } ?? profiles.first
    }
    var headphoneName: String {
        if let name = state.name, !name.isEmpty { return name }
        if let device = devices.first(where: { BluetoothAddress.normalize($0.address) == state.address }) { return device.displayName }
        if !AppPreferences.shared.lastDeviceName.isEmpty { return AppPreferences.shared.lastDeviceName }
        return "你的漫步者耳机"
    }
    var audioLabel: String {
        switch state.audio {
        case "connected": return "系统音频已就绪"
        case "connecting": return "系统音频连接中"
        case "disconnected": return "系统音频未连接"
        default: return "系统音频待确认"
        }
    }

    func start() {
        guard !hasStarted else { return }
        hasStarted = true
        startupTask = Task {
            do {
                let (version, profiles) = try await native.prepare()
                guard !shuttingDown else { return }
                coreVersion = version
                self.profiles = profiles
                if !profiles.contains(where: { $0.id == AppPreferences.shared.selectedProfile }) {
                    AppPreferences.shared.selectedProfile = profiles.first?.id ?? "basedevice"
                }
                ready = true
                record("耳机服务已启动", detail: "核心版本 \(version)")
                beginPolling()
                do {
                    if let secret = try GroupSecret.load(), !secret.isEmpty {
                        groupSecret = secret
                        rememberGroup = true
                        if AppPreferences.shared.autoJoinGroup { joinGroup() }
                    }
                } catch { report("无法读取已保存的组名", error: error) }
            } catch { report("耳机服务启动失败", error: error) }
        }
    }

    func scan() {
        guard ready, !scanning, !busy else { return }
        scanning = true
        run("正在查找已配对的耳机") {
            defer { self.scanning = false }
            self.devices = try await self.native.scan().sorted {
                if $0.isEdifier != $1.isEdifier { return $0.isEdifier }
                return $0.displayName.localizedStandardCompare($1.displayName) == .orderedAscending
            }
            self.record("设备列表已更新", detail: "发现 \(self.devices.count) 台已配对设备")
        }
    }

    func connect(_ device: HeadphoneDevice) {
        connect(address: device.address, name: device.displayName)
    }

    func reconnect() {
        let prefs = AppPreferences.shared
        connect(address: prefs.lastDeviceAddress, name: prefs.lastDeviceName)
    }

    func connect(address: String, name: String = "") {
        guard let address = BluetoothAddress.normalize(address) else {
            notify("地址格式不正确", "请输入类似 AA:BB:CC:DD:EE:FF 的蓝牙地址.", error: true)
            return
        }
        run("正在连接耳机") {
            try await self.native.connect(address)
            var next = SessionState()
            next.connected = true
            next.address = address
            next.name = name.isEmpty ? nil : name
            self.state = next
            self.synchronizeConnection()
            let prefs = AppPreferences.shared
            prefs.lastDeviceAddress = address
            prefs.lastDeviceName = name
            if let profile = self.profiles.sorted(by: { $0.id.count > $1.id.count }).first(where: {
                $0.id != "basedevice" && name.lowercased().replacingOccurrences(of: " ", with: "").contains($0.id)
            }) { prefs.selectedProfile = profile.id }
            self.notify("控制通道已连接", "正在读取耳机状态. 音频连接结果会单独显示.")
            try await self.native.readout(prefs.selectedProfile)
        }
    }

    func disconnect() {
        run("正在断开控制通道") {
            try await self.native.disconnect()
            self.state.connected = false
            self.state.clearReadings()
            self.synchronizeConnection()
            self.notify("控制通道已断开", "系统音频保持当前连接. 如需转移声音, 请使用跨设备交接.")
        }
    }

    func refreshReadings() {
        guard isConnected else { return }
        run("正在读取耳机状态") {
            try await self.native.readout(AppPreferences.shared.selectedProfile)
            self.record("已请求读取状态")
        }
    }

    func selectProfile(_ id: String) {
        AppPreferences.shared.selectedProfile = id
        state.clearReadings()
        if isConnected { refreshReadings() }
    }

    func send(_ op: String, values: [String: Any] = [:], label: String, query: String? = nil) {
        guard canControl else { return }
        if op == "set_name", let name = values["name"] as? String,
           name.utf8.count > (selectedProfile?.maxNameLen ?? 24) {
            notify("耳机名称过长", "当前机型最多支持 \(selectedProfile?.maxNameLen ?? 24) 个 UTF-8 字节.", error: true)
            return
        }
        if op == "set_control_settings", ["normal", "reduction", "ambient"].filter({ values[$0] as? Bool == true }).count < 2 {
            notify("请选择至少两种模式", "耳机按键需要至少两种模式来循环切换.", error: true)
            return
        }
        run(label) {
            var command = values
            command["op"] = op
            let bytes = try JSONSerialization.data(withJSONObject: command)
            try await self.native.send(String(decoding: bytes, as: UTF8.self))
            self.notify("指令已发送", "\(label). 状态以耳机回报为准.")
            if let query {
                let bytes = try JSONSerialization.data(withJSONObject: ["op": query])
                try await self.native.send(String(decoding: bytes, as: UTF8.self))
            }
        }
    }

    func loadRememberedGroup() {
        do {
            groupSecret = try GroupSecret.load() ?? ""
            rememberGroup = !groupSecret.isEmpty
        } catch { report("无法读取已保存的组名", error: error) }
    }

    func joinGroup() {
        guard !groupJoined else { return }
        let secret = groupSecret
        guard !secret.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            notify("请先输入组名", "在手机和电脑上使用完全相同的组名.", error: true)
            return
        }
        run("正在加入交接组") {
            try await self.native.join(secret)
            self.groupJoined = true
            self.notify("已加入交接组", "同一网络中使用相同组名的设备会自动出现在这里.")
            do {
                if self.rememberGroup { try GroupSecret.save(secret) }
                else {
                    try GroupSecret.delete()
                    AppPreferences.shared.autoJoinGroup = false
                }
            } catch { self.report("已入组, 但无法保存组名", error: error) }
        }
    }

    func leaveGroup() {
        run("正在退出交接组", allowDuringHandoff: true) {
            try await self.native.leave()
            self.groupJoined = false
            self.peers = []
            self.state.handoff = nil
            self.state.holding = nil
            self.handoffStarted = nil
            self.claimTarget = nil
            self.notify("已退出交接组", "本机耳机控制仍可继续使用.")
        }
    }

    func claim(_ peer: GroupPeer) {
        guard groupJoined, peer.canAudio, let address = peer.holding, !address.isEmpty else { return }
        beginClaim(address: address, label: peer.displayName) { try await self.native.claimPeer(peer.id) }
    }

    func claim(address: String) {
        guard groupJoined, let normalized = BluetoothAddress.normalize(address) else {
            notify("无法请求交接", "请先加入组并输入有效的耳机蓝牙地址.", error: true)
            return
        }
        beginClaim(address: normalized, label: "指定耳机") { try await self.native.claimAddress(normalized) }
    }

    private func beginClaim(address: String, label: String, action: @escaping () async throws -> Void) {
        run("正在请求接管") {
            self.claimTarget = BluetoothAddress.normalize(address)
            self.state.handoff = HandoffProgress(kind: "requesting")
            self.handoffStarted = Date()
            do {
                try await action()
                self.record("请求接管耳机", detail: label)
            } catch {
                self.claimTarget = nil
                self.handoffStarted = nil
                self.state.handoff = HandoffProgress(kind: "failed", reason: error.localizedDescription)
                throw error
            }
        }
    }

    func inspect(frame: Bool) {
        run("正在解析") {
            self.diagnosticOutput = try await self.native.inspect(self.diagnosticInput, frame: frame)
        }
    }

    func clearActivity() { activities.removeAll() }

    func shutdown() async {
        guard !shuttingDown else { return }
        shuttingDown = true
        pollTask?.cancel()
        startupTask?.cancel()
        operationTask?.cancel()
        await startupTask?.value
        await operationTask?.value
        await pollTask?.value
        await native.shutdown()
        ready = false
        record("耳机服务已停止")
    }

    private func beginPolling() {
        pollTask = Task { [weak self] in
            var tick = 0
            while !Task.isCancelled {
                guard let self, !self.shuttingDown else { return }
                do {
                    let snapshot = try await self.native.poll(groupJoined: self.groupJoined, includePeers: tick % 8 == 0)
                    guard !Task.isCancelled else { return }
                    self.consume(snapshot)
                    self.hasReportedPollFailure = false
                } catch {
                    if !self.hasReportedPollFailure && !self.shuttingDown {
                        self.report("状态同步失败", error: error)
                        self.hasReportedPollFailure = true
                    }
                }
                tick += 1
                try? await Task.sleep(nanoseconds: 250_000_000)
            }
        }
    }

    private func consume(_ snapshot: NativeSnapshot) {
        if groupJoined, let peers = snapshot.peers { self.peers = peers.sorted { $0.displayName < $1.displayName } }
        for event in snapshot.events {
            if event.kind == "handoff" && !groupJoined { continue }
            state.apply(event)
            switch event.kind {
            case "bt_state": record(event.connected == true ? "控制通道已连接" : "控制通道已断开")
            case "handoff":
                if let progress = event.progress {
                    if progress.isActive && handoffStarted == nil { handoffStarted = Date() }
                    record(progress.title, detail: progress.reason ?? "", error: progress.kind == "failed")
                    if !progress.isActive { handoffStarted = nil }
                    if progress.kind == "failed" || progress.kind == "busy" {
                        claimTarget = nil
                        notify(progress.title, progress.reason ?? "请稍后重新尝试.", error: true)
                    }
                }
            case "message":
                if let text = event.text { record("耳机服务", detail: text) }
            default: break
            }
        }
        state.holding = snapshot.holding
        if state.holding != nil { state.audio = "connected" }
        else if state.audio == "connected" { state.audio = "unknown" }
        synchronizeConnection()
        if state.handoff?.kind == "done", let target = claimTarget, state.holding == target, !busy {
            claimTarget = nil
            if !isConnected || state.address != target { connect(address: target) }
        }
        if let started = handoffStarted, Date().timeIntervalSince(started) > 35 {
            state.handoff = HandoffProgress(kind: "failed", reason: "未能在预期时间内确认音频连接. 请检查两端蓝牙状态后重试.")
            handoffStarted = nil
            claimTarget = nil
            notify("交接超时", state.handoff?.reason ?? "", error: true)
        }
    }

    private func synchronizeConnection() {
        isConnected = state.connected
        connectionLabel = isConnected ? "\(headphoneName) · 已连接" : "尚未连接耳机"
    }

    private func run(_ label: String, allowDuringHandoff: Bool = false, action: @escaping @MainActor () async throws -> Void) {
        guard ready, operation == nil, (allowDuringHandoff || state.handoff?.isActive != true), !shuttingDown else { return }
        operation = label
        record(label)
        operationTask = Task {
            defer { operation = nil }
            do { try await action() }
            catch { if !shuttingDown { report(label + "失败", error: error) } }
        }
    }

    private func notify(_ title: String, _ detail: String, error: Bool = false) {
        notice = UserNotice(title: title, detail: detail, isError: error)
        record(title, detail: detail, error: error)
    }

    private func report(_ title: String, error: Error) { notify(title, error.localizedDescription, error: true) }

    private func record(_ title: String, detail: String = "", error: Bool = false) {
        activities.insert(ActivityEntry(title: title, detail: detail, isError: error), at: 0)
        if activities.count > 200 { activities.removeLast(activities.count - 200) }
        let message = detail.isEmpty ? title : "\(title): \(detail)"
        if error { AppLog.error(message) } else { AppLog.info(message) }
    }
}
