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
    private var presentationTask: Task<Void, Never>?
    private var presentationGeneration = 0
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
        // 启动隐藏时只创建后台服务, 普通主窗口延迟到恢复 regular 身份后创建.
        wantsVisibleWindow = !preferences.startHidden || pendingActivation
        updateDockPolicy()
        let model = AppModel()
        self.model = model
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
            cancelWindowPresentation()
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

    /// 唯一的主窗口显示入口. 应用身份切换与窗口创建分开调度, 隐藏或退出会取消迟到的显示.
    private func showMainWindow(reason: String) {
        guard ready, !terminating else { return }
        wantsVisibleWindow = true
        suppressActivationUntil = 0
        cancelWindowPresentation()
        guard updateDockPolicy() else { return }
        let generation = presentationGeneration
        presentationTask = Task { [weak self] in
            // setActivationPolicy 返回不代表 Dock 和 WindowServer 已处理完身份切换.
            // 先让出主事件循环, 再创建/显示普通窗口, 随后核验系统实际激活结果.
            await withCheckedContinuation { continuation in
                DispatchQueue.main.async { continuation.resume() }
            }
            guard let self, self.mayPresentWindow(generation) else { return }
            guard NSApp.activationPolicy() == .regular, let model = self.model, let preferences = self.preferences else {
                AppLog.error("普通应用身份尚未就绪, 暂不创建或显示主窗口.", category: "desktop")
                self.presentationTask = nil
                return
            }
            if self.mainWindow == nil {
                self.mainWindow = MainWindowController(model: model, preferences: preferences) { [weak self] in
                    self?.enqueue(.hideWindow)
                }
                AppLog.debug("已以普通应用身份创建唯一主窗口.", category: "desktop")
            }
            NSApp.unhide(nil)
            self.mainWindow?.reveal()
            NSApp.activate(ignoringOtherApps: true)
            // 激活和 Space 动画异步完成. 只在这次显式唤起尚未完成时重试一次,
            // 成功后立即停止, 不在后台持续抢焦点, 也不更改已有窗口的全屏状态.
            for attempt in 0..<20 {
                guard self.mayPresentWindow(generation) else { return }
                if let window = self.mainWindow?.window, NSApp.isActive,
                   window.isVisible, !window.isMiniaturized, window.isKeyWindow, window.isOnActiveSpace {
                    AppLog.info("显示主窗口: \(reason), 全屏=\(window.styleMask.contains(.fullScreen)).", category: "desktop")
                    self.presentationTask = nil
                    return
                }
                if attempt == 5 {
                    self.mainWindow?.reveal()
                    NSApp.activate(ignoringOtherApps: true)
                }
                do { try await Task.sleep(nanoseconds: 100_000_000) }
                catch { return }
            }
            AppLog.error("主窗口唤起未在预期时间内取得当前 Space 的键盘焦点: \(reason).", category: "desktop")
            self.presentationTask = nil
        }
    }

    private func mayPresentWindow(_ generation: Int) -> Bool {
        !Task.isCancelled && ready && !terminating && wantsVisibleWindow && generation == presentationGeneration
    }

    private func cancelWindowPresentation() {
        presentationGeneration &+= 1
        presentationTask?.cancel()
        presentationTask = nil
    }

    private func hideMainWindow(reason: String) {
        guard ready, !terminating else { return }
        wantsVisibleWindow = false
        cancelWindowPresentation()
        suppressActivationBriefly()
        mainWindow?.hide()
        NSApp.hide(nil)
        updateDockPolicy()
        AppLog.info("隐藏主窗口并继续后台运行: \(reason).", category: "desktop")
    }

    private func suppressActivationBriefly() {
        suppressActivationUntil = ProcessInfo.processInfo.systemUptime + 0.8
    }

    @discardableResult
    private func updateDockPolicy() -> Bool {
        let hideDock = !wantsVisibleWindow && (preferences?.hideDockWhenHidden ?? true)
        let policy: NSApplication.ActivationPolicy = hideDock ? .accessory : .regular
        guard NSApp.activationPolicy() != policy else { return true }
        guard NSApp.setActivationPolicy(policy) else {
            AppLog.error("无法切换 Dock 可见性, 目标隐藏状态 \(hideDock).", category: "desktop")
            return false
        }
        return true
    }
}
