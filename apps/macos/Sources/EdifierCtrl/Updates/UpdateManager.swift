import AppKit
import Combine
import Foundation
import UserNotifications

@MainActor
final class UpdateManager: ObservableObject {
    static let shared = UpdateManager()

    enum Phase: Equatable {
        case idle, checking, upToDate, available, downloading, readyToRestart, handedOff, dmgOpened
        case failed(String)
    }

    @Published var isPresented = false
    @Published private(set) var phase: Phase = .idle
    @Published private(set) var candidate: UpdateCandidate?
    @Published private(set) var received: Int64 = 0
    @Published private(set) var total: Int64 = 0
    @Published private(set) var installationFailure: String?
    @Published private(set) var notificationsAuthorized = false
    private let notificationDelegate = UpdateNotificationDelegate()
    private var startupTask: Task<Void, Never>?
    private var operation: Task<Void, Never>?
    private var download: UpdateDownload?
    private var archive: URL?
    private var started = false
    private var stopping = false

    var hasUpdate: Bool { candidate != nil }
    var statusTitle: String {
        if let candidate { return "新版本 \(candidate.release.tagName) 可用" }
        return AppVersion.display
    }
    var isBusy: Bool { phase == .checking || phase == .downloading || phase == .handedOff }

    func start() {
        guard !started else { return }
        started = true
        stopping = false
        if Bundle.main.bundleURL.pathExtension == "app" {
            UNUserNotificationCenter.current().delegate = notificationDelegate
        }
        startupTask = Task {
            do {
                let dataDirectory = AppPaths.dataDirectory
                let result = try await Task.detached(priority: .utility) {
                    let directory = try UpdateFiles.directory(dataDirectory: dataDirectory)
                    return try UpdateInstaller.previousFailure(directory: directory)
                }.value
                if let result {
                    installationFailure = result
                    phase = .failed(result)
                    isPresented = true
                    AppLog.error("上次更新未完成: \(result)")
                }
                try await Task.sleep(nanoseconds: 5_000_000_000)
                let bundle = UpdateInstaller.installedBundle()
                let mayCleanBackup = installationFailure == nil
                await Task.detached(priority: .utility) {
                    if let directory = try? UpdateFiles.directory(dataDirectory: dataDirectory),
                       UpdateInstaller.cleanPreviousArtifacts(directory: directory), mayCleanBackup {
                        UpdateInstaller.cleanPreviousBackup(bundle: bundle)
                    }
                }.value
                guard !Task.isCancelled, installationFailure == nil, AppPreferences.shared.autoCheckUpdates else { return }
                checkForUpdates(userInitiated: false)
            } catch is CancellationError {} catch {
                AppLog.debug("更新启动检查暂缓: \(error.localizedDescription)")
            }
        }
    }

    func checkForUpdates(userInitiated: Bool) {
        if userInitiated { isPresented = true }
        guard !stopping, operation == nil, phase != .handedOff, phase != .downloading else { return }
        if userInitiated { installationFailure = nil }
        guard installationFailure == nil else { return }
        guard let current = UpdateVersion(AppVersion.display) else {
            phase = .idle
            return
        }
        phase = .checking
        candidate = nil
        archive = nil
        AppLog.info("检查更新, 当前版本 \(AppVersion.display)")
        operation = Task {
            defer { operation = nil }
            let session = UpdateHTTP.session()
            defer { session.invalidateAndCancel() }
            do {
                let repository = try UpdateValidation.repository(Bundle.main.object(forInfoDictionaryKey: "EdifierReleaseRepository") as? String)
                let endpoint = URL(string: "https://api.github.com/repos/\(repository)/releases/latest")!
                let data = try await UpdateHTTP.data(from: endpoint, session: session, limit: 2 * 1024 * 1024)
                let release = try JSONDecoder().decode(UpdateRelease.self, from: data)
                guard !release.draft, !release.prerelease, let version = UpdateVersion(release.tagName) else {
                    throw UpdateFailure("GitHub 没有返回有效的正式版本.")
                }
                guard version > current else {
                    phase = .upToDate
                    AppLog.info("当前已是最新版本")
                    return
                }
                let name = try UpdateValidation.assetName(tag: release.tagName)
                let archives = release.assets.filter { $0.name == name }
                let checksums = release.assets.filter { $0.name == "SHA256SUMS" }
                guard archives.count == 1, checksums.count == 1,
                      let asset = archives.first, let sums = checksums.first,
                      asset.size > 0, asset.size <= 4 * 1024 * 1024 * 1024 else {
                    throw UpdateFailure("Release 缺少当前架构的唯一安装包或 SHA256SUMS.")
                }
                try UpdateValidation.assetURL(asset.browserDownloadURL, repository: repository, tag: release.tagName, name: name)
                try UpdateValidation.assetURL(sums.browserDownloadURL, repository: repository, tag: release.tagName, name: "SHA256SUMS")
                try UpdateValidation.releaseURL(release.htmlURL, repository: repository, tag: release.tagName)
                let checksumData = try await UpdateHTTP.data(from: sums.browserDownloadURL, session: session, limit: 1024 * 1024)
                guard let text = String(data: checksumData, encoding: .utf8) else { throw UpdateFailure("SHA256SUMS 不是有效的文本文件.") }
                let digest = try UpdateValidation.checksum(text, named: name)
                let candidate = UpdateCandidate(release: release, archive: asset, checksum: sums, digest: digest)
                let directory = try UpdateFiles.directory(dataDirectory: AppPaths.dataDirectory)
                let cached = try await UpdateFiles.verifiedArchive(candidate, directory: directory)
                try Task.checkCancellation()
                self.candidate = candidate
                archive = cached
                phase = cached == nil ? .available : .readyToRestart
                AppLog.info("发现新版本 \(release.tagName), 已校验缓存: \(cached != nil)")
                if !userInitiated && AppPreferences.shared.skippedUpdateVersion != release.tagName {
                    await notify(release.tagName)
                }
            } catch is CancellationError {} catch {
                guard !Task.isCancelled else { return }
                phase = .failed(error.localizedDescription)
                if userInitiated { AppLog.error("手动检查更新失败: \(error.localizedDescription)") }
                else { AppLog.debug("静默检查更新失败: \(error.localizedDescription)") }
            }
        }
    }

    func downloadUpdate() {
        guard !stopping, operation == nil, let candidate, phase != .handedOff else { return }
        phase = .downloading
        received = 0
        total = candidate.archive.size
        AppLog.info("开始下载更新 \(candidate.archive.name)")
        operation = Task {
            defer { operation = nil; download = nil }
            do {
                let directory = try UpdateFiles.directory(dataDirectory: AppPaths.dataDirectory)
                if let cached = try await UpdateFiles.verifiedArchive(candidate, directory: directory) {
                    try Task.checkCancellation()
                    archive = cached
                    phase = .readyToRestart
                    AppLog.info("恢复已校验的更新包, 等待用户重启")
                    return
                }
                let downloader = UpdateDownload(candidate: candidate, directory: directory) { [weak self] received, total in
                    Task { @MainActor in
                        guard self?.phase == .downloading else { return }
                        self?.received = received
                        self?.total = total
                    }
                }
                download = downloader
                let file = try await downloader.run()
                try Task.checkCancellation()
                archive = file
                phase = .readyToRestart
                AppLog.info("更新已下载并通过 SHA256 校验, 等待用户重启")
            } catch is CancellationError {
                phase = .available
                AppLog.info("更新下载已取消, 保留部分文件供续传")
            } catch {
                phase = .failed(error.localizedDescription)
                AppLog.error("更新下载失败: \(error.localizedDescription)")
            }
        }
    }

    func cancelDownload() {
        guard phase == .downloading else { return }
        operation?.cancel()
        download?.cancel()
    }

    func skipVersion() {
        guard let candidate, !isBusy else { return }
        AppPreferences.shared.skippedUpdateVersion = candidate.release.tagName
        AppLog.info("用户跳过更新 \(candidate.release.tagName)")
        isPresented = false
    }

    // 仅由更新窗口中的确认按钮调用. 下载完成不会进入这里.
    func restartAndInstall() {
        guard !stopping, operation == nil, phase == .readyToRestart, let archive, let candidate else { return }
        AppLog.info("用户确认安装更新 \(candidate.release.tagName)")
        operation = Task {
            defer { operation = nil }
            do {
                let digest = try await UpdateFiles.hashAsync(archive)
                try Task.checkCancellation()
                guard digest == candidate.digest else { throw UpdateFailure("安装包在校验后发生变化, 请重新下载.") }
                guard let bundle = UpdateInstaller.installedBundle() else {
                    guard NSWorkspace.shared.open(archive) else { throw UpdateFailure("无法打开 DMG, 请在更新目录手动打开安装包.") }
                    phase = .dmgOpened
                    return
                }
                let dataDirectory = AppPaths.dataDirectory
                let logFile = AppPaths.logFile
                let directory = try UpdateFiles.directory(dataDirectory: dataDirectory)
                try await Task.detached(priority: .utility) {
                    try UpdateInstaller.handOff(archive: archive, digest: digest, version: candidate.release.tagName, directory: directory, bundle: bundle, dataDirectory: dataDirectory, logFile: logFile)
                }.value
                phase = .handedOff
                AppLog.info("更新已交给独立助手, 请求统一优雅退出")
                NotificationCenter.default.post(name: .edifierQuitRequested, object: nil)
            } catch {
                phase = .failed(error.localizedDescription)
                AppLog.error("安装交接失败: \(error.localizedDescription)")
            }
        }
    }

    func stop() async {
        stopping = true
        startupTask?.cancel()
        operation?.cancel()
        download?.cancel()
        await startupTask?.value
        await operation?.value
        startupTask = nil
        AppLog.debug("更新服务已停止")
    }

    func requestNotificationPermission() {
        guard Bundle.main.bundleURL.pathExtension == "app" else { return }
        Task {
            do {
                notificationsAuthorized = try await UNUserNotificationCenter.current().requestAuthorization(options: [.alert])
                AppLog.info("更新通知授权: \(notificationsAuthorized)")
            } catch { AppLog.error("更新通知授权失败: \(error.localizedDescription)") }
        }
    }

    func refreshNotificationPermission() async {
        guard Bundle.main.bundleURL.pathExtension == "app" else { return }
        let settings = await UNUserNotificationCenter.current().notificationSettings()
        notificationsAuthorized = settings.authorizationStatus == .authorized || settings.authorizationStatus == .provisional
    }

    private func notify(_ version: String) async {
        guard Bundle.main.bundleURL.pathExtension == "app" else { return }
        let center = UNUserNotificationCenter.current()
        let settings = await center.notificationSettings()
        guard settings.authorizationStatus == .authorized || settings.authorizationStatus == .provisional else { return }
        let content = UNMutableNotificationContent()
        content.title = "EdifierCtrl 有新版本"
        content.body = "\(version) 已发布. 打开主窗口查看更新."
        do { try await center.add(UNNotificationRequest(identifier: "EdifierCtrl.update", content: content, trigger: nil)) }
        catch { AppLog.debug("更新通知未发送: \(error.localizedDescription)") }
    }
}
