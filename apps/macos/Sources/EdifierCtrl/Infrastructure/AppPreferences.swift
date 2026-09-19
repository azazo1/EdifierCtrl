import Combine
import Foundation

@MainActor
final class AppPreferences: ObservableObject {
    static let shared = AppPreferences()

    @Published var installationID = UUID().uuidString { didSet { changed("installationID") } }
    @Published var startHidden = false { didSet { changed("startHidden") } }
    @Published var hideDockWhenHidden = true { didSet { changed("hideDockWhenHidden") } }
    @Published var autoJoinGroup = false { didSet { changed("autoJoinGroup") } }
    @Published var verboseLogging = false {
        didSet {
            AppLog.setVerbose(verboseLogging)
            changed("verboseLogging")
        }
    }
    @Published var selectedProfile = "basedevice" { didSet { changed("selectedProfile") } }
    @Published var lastDeviceAddress = "" { didSet { changed("lastDeviceAddress") } }
    @Published var lastDeviceName = "" { didSet { changed("lastDeviceName") } }
    @Published var windowWidth = 1120.0 { didSet { changed("windowWidth") } }
    @Published var windowHeight = 780.0 { didSet { changed("windowHeight") } }
    @Published var windowZoomed = false { didSet { changed("windowZoomed") } }
    @Published var autoCheckUpdates = true { didSet { changed("autoCheckUpdates") } }
    @Published var skippedUpdateVersion = "" { didSet { changed("skippedUpdateVersion") } }
    @Published private(set) var persistenceError: String?

    private var applyingDocument = false
    private var writable = true
    private var saveTask: Task<Void, Never>?

    private init() {
        applyingDocument = true
        do {
            try AppPaths.prepareDataDirectory()
            if FileManager.default.fileExists(atPath: AppPaths.preferencesFile.path) {
                let data = try Data(contentsOf: AppPaths.preferencesFile)
                apply(try PreferencesMigration.decode(data))
            }
        } catch {
            // 无法解码时禁写, 避免默认值覆盖原设置或较新版本的文档.
            writable = false
            recordFailure(error, operation: "读取设置")
        }
        applyingDocument = false
        AppLog.setVerbose(verboseLogging)
        if writable { flush() }
    }

    func storeWindowState(width: Double, height: Double, zoomed: Bool) {
        applyingDocument = true
        if width.isFinite && width > 0 { windowWidth = width }
        if height.isFinite && height > 0 { windowHeight = height }
        windowZoomed = zoomed
        applyingDocument = false
        flush()
    }

    /// 窗口调整结束和退出时使用同步原子写入, 不等待防抖计时器.
    func flush() {
        saveTask?.cancel()
        saveTask = nil
        guard writable else { return }
        do {
            try AppPaths.prepareDataDirectory()
            let encoder = JSONEncoder()
            encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
            let data = try encoder.encode(document)
            try data.write(to: AppPaths.preferencesFile, options: .atomic)
            try FileManager.default.setAttributes([.posixPermissions: 0o600], ofItemAtPath: AppPaths.preferencesFile.path)
            persistenceError = nil
            AppLog.debug("设置已保存.", category: "preferences")
        } catch {
            recordFailure(error, operation: "保存设置")
        }
    }

    private func changed(_ key: String) {
        guard !applyingDocument else { return }
        AppLog.info("设置已更改: \(key).", category: "preferences")
        saveTask?.cancel()
        saveTask = Task { [weak self] in
            do { try await Task.sleep(nanoseconds: 250_000_000) }
            catch { return }
            guard !Task.isCancelled else { return }
            self?.flush()
        }
    }

    private func recordFailure(_ error: Error, operation: String) {
        let message = "\(operation)失败: \(error.localizedDescription)"
        persistenceError = message
        AppLog.error(message, category: "preferences")
    }

    private func apply(_ value: PreferencesDocument) {
        installationID = value.installationID
        startHidden = value.startHidden
        hideDockWhenHidden = value.hideDockWhenHidden
        autoJoinGroup = value.autoJoinGroup
        verboseLogging = value.verboseLogging
        selectedProfile = value.selectedProfile
        lastDeviceAddress = value.lastDeviceAddress
        lastDeviceName = value.lastDeviceName
        windowWidth = value.windowWidth
        windowHeight = value.windowHeight
        windowZoomed = value.windowZoomed
        autoCheckUpdates = value.autoCheckUpdates
        skippedUpdateVersion = value.skippedUpdateVersion
    }

    private var document: PreferencesDocument {
        var value = PreferencesDocument()
        value.installationID = installationID
        value.startHidden = startHidden
        value.hideDockWhenHidden = hideDockWhenHidden
        value.autoJoinGroup = autoJoinGroup
        value.verboseLogging = verboseLogging
        value.selectedProfile = selectedProfile
        value.lastDeviceAddress = lastDeviceAddress
        value.lastDeviceName = lastDeviceName
        value.windowWidth = windowWidth
        value.windowHeight = windowHeight
        value.windowZoomed = windowZoomed
        value.autoCheckUpdates = autoCheckUpdates
        value.skippedUpdateVersion = skippedUpdateVersion
        return value
    }
}
