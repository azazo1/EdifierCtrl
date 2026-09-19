import AppKit
import Combine

@MainActor
final class DesktopApplicationDelegate: NSObject, NSApplicationDelegate {
    private let instance = SingleInstanceController()
    private let signals = TerminationSignals()
    private var model: AppModel?
    private var mainWindow: MainWindowController?
    private var statusItem: StatusItemController?
    private var preferences: AppPreferences?
    private var subscriptions: Set<AnyCancellable> = []
    private var updateQuitObserver: NSObjectProtocol?
    private var updateShowWindowObserver: NSObjectProtocol?
    private var terminationTask: Task<Void, Never>?
    private var ready = false
    private var terminating = false
    private var shutdownComplete = false
    private var wantsVisibleWindow = false
    private var pendingActivation = false
    private var suppressActivationUntil: TimeInterval = 0

    func applicationDidFinishLaunching(_ notification: Notification) {
        do {
            guard try instance.acquire(onActivation: { [weak self] arguments in
                AppLog.info("收到二次启动请求, 参数数 \(arguments.count).", category: "desktop")
                guard let self else { return }
                if self.ready { self.enqueue(.showWindow("二次启动")) }
                else { self.pendingActivation = true }
            }) else {
                NSApp.terminate(nil)
                return
            }
        } catch {
            AppLog.error("无法取得数据目录实例锁: \(error.localizedDescription)", category: "desktop")
            AppLog.flush()
            NSApp.terminate(nil)
            return
        }

        let preferences = AppPreferences.shared
        self.preferences = preferences
        AppLog.installExceptionHandler()
        AppLog.info("EdifierCtrl \(AppVersion.display) 启动, fake=\(AppVersion.isFakeBuild), 系统 \(ProcessInfo.processInfo.operatingSystemVersionString).", category: "desktop")
        let model = AppModel()
        self.model = model
        mainWindow = MainWindowController(model: model, preferences: preferences) { [weak self] in
            self?.enqueue(.hideWindow)
        }
        statusItem = StatusItemController(model: model, preferences: preferences, menuTrackingEnded: { [weak self] in
            guard let self, !self.wantsVisibleWindow else { return }
            self.suppressActivationBriefly()
        }) { [weak self] command in
            self?.enqueue(command)
        }
        signals.start { [weak self] reason in self?.enqueue(.quit(reason)) }
        preferences.$hideDockWhenHidden
            .dropFirst()
            .receive(on: RunLoop.main)
            .sink { [weak self] _ in
                guard let self, !self.terminating else { return }
                if !self.wantsVisibleWindow { self.suppressActivationBriefly() }
                self.updateDockPolicy()
            }
            .store(in: &subscriptions)
        updateQuitObserver = NotificationCenter.default.addObserver(
            forName: .edifierQuitRequested,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            Task { @MainActor in self?.enqueue(.quit("安装更新")) }
        }
        updateShowWindowObserver = NotificationCenter.default.addObserver(
            forName: .edifierShowWindowRequested,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            Task { @MainActor in self?.enqueue(.showWindow("更新通知")) }
        }
        ready = true
        model.start()
        UpdateManager.shared.start()
        if preferences.startHidden && !pendingActivation {
            hideMainWindow(reason: "启动时隐藏")
        } else {
            showMainWindow(reason: "启动")
        }
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool { false }

    func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows flag: Bool) -> Bool {
        guard ready, !terminating else { return false }
        guard ProcessInfo.processInfo.systemUptime >= suppressActivationUntil else {
            AppLog.debug("忽略隐藏后的 reopen 回弹.", category: "desktop")
            return false
        }
        enqueue(.showWindow("Dock 或 Finder reopen"))
        return false
    }

    func applicationDidBecomeActive(_ notification: Notification) {
        guard ready, !terminating, !wantsVisibleWindow, statusItem?.isTrackingMenu != true else { return }
        guard ProcessInfo.processInfo.systemUptime >= suppressActivationUntil else {
            AppLog.debug("忽略隐藏后的 activation 回弹.", category: "desktop")
            return
        }
        enqueue(.showWindow("应用激活"))
    }

    func applicationShouldTerminate(_ sender: NSApplication) -> NSApplication.TerminateReply {
        guard ready else {
            instance.release()
            AppLog.flush()
            return .terminateNow
        }
        if shutdownComplete { return .terminateNow }
        if terminationTask == nil {
            terminating = true
            AppLog.info("开始优雅退出.", category: "desktop")
            mainWindow?.prepareForTermination()
            preferences?.flush()
            terminationTask = Task { [weak self] in
                guard let self else { return }
                let started = ProcessInfo.processInfo.systemUptime
                AppLog.info("正在停止更新服务.", category: "desktop")
                await UpdateManager.shared.stop()
                AppLog.info("更新服务已停止, 正在关闭耳机服务.", category: "desktop")
                await self.model?.shutdown()
                self.preferences?.flush()
                self.statusItem?.remove()
                self.signals.stop()
                AppLog.info("服务已停止, 退出耗时 \(String(format: "%.2f", ProcessInfo.processInfo.systemUptime - started)) 秒.", category: "desktop")
                AppLog.flush()
                self.shutdownComplete = true
                NSApp.terminate(nil)
            }
        }
        // 先返回主事件循环让 MainActor 清理任务执行, 完成后再次请求终止.
        // terminateLater 会在同步 terminate 调用内进入嵌套循环, 阻止该任务调度.
        return .terminateCancel
    }

    func applicationWillTerminate(_ notification: Notification) {
        AppLog.info("应用即将终止.", category: "desktop")
        if let updateQuitObserver { NotificationCenter.default.removeObserver(updateQuitObserver) }
        if let updateShowWindowObserver { NotificationCenter.default.removeObserver(updateShowWindowObserver) }
        subscriptions.removeAll()
        AppLog.flush()
        instance.release()
    }

    private func enqueue(_ command: DesktopCommand) {
        Task { @MainActor [weak self] in self?.handle(command) }
    }

    private func handle(_ command: DesktopCommand) {
        guard !terminating else { return }
        switch command {
        case let .showWindow(reason): showMainWindow(reason: reason)
        case .hideWindow: hideMainWindow(reason: "关闭按钮或快捷键")
        case .checkUpdates:
            AppLog.info("用户请求检查更新.", category: "desktop")
            showMainWindow(reason: "检查更新")
            UpdateManager.shared.isPresented = true
            UpdateManager.shared.checkForUpdates(userInitiated: true)
        case .toggleAutomaticUpdates:
            preferences?.autoCheckUpdates.toggle()
            preferences?.flush()
        case let .quit(reason):
            AppLog.info("收到退出请求: \(reason).", category: "desktop")
            NSApp.terminate(nil)
        }
    }

    /// 所有主动唤起入口只访问已创建的窗口, 先恢复 Dock 再聚焦窗口.
    private func showMainWindow(reason: String) {
        guard ready, !terminating, mainWindow != nil else { return }
        wantsVisibleWindow = true
        suppressActivationUntil = 0
        updateDockPolicy()
        NSApp.unhide(nil)
        mainWindow?.reveal()
        NSApp.activate(ignoringOtherApps: true)
        AppLog.info("显示主窗口: \(reason).", category: "desktop")
    }

    private func hideMainWindow(reason: String) {
        guard ready, !terminating else { return }
        wantsVisibleWindow = false
        suppressActivationBriefly()
        mainWindow?.hide()
        NSApp.hide(nil)
        updateDockPolicy()
        AppLog.info("隐藏主窗口并继续后台运行: \(reason).", category: "desktop")
    }

    private func suppressActivationBriefly() {
        suppressActivationUntil = ProcessInfo.processInfo.systemUptime + 0.8
    }

    private func updateDockPolicy() {
        let hideDock = !wantsVisibleWindow && (preferences?.hideDockWhenHidden ?? true)
        let policy: NSApplication.ActivationPolicy = hideDock ? .accessory : .regular
        guard NSApp.activationPolicy() != policy else { return }
        if !NSApp.setActivationPolicy(policy) {
            AppLog.error("无法切换 Dock 可见性, 目标隐藏状态 \(hideDock).", category: "desktop")
        }
    }
}
