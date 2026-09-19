import AppKit
import SwiftUI

@MainActor
final class MainWindowController: NSWindowController, NSWindowDelegate {
    private let preferences: AppPreferences
    private let requestHide: () -> Void
    private var normalSize: NSSize
    private var restoring = true
    private var fullscreenTransition = false
    private var zoomTransition = false
    private var resizeTask: Task<Void, Never>?
    private var zoomTask: Task<Void, Never>?

    init(model: AppModel, preferences: AppPreferences, requestHide: @escaping () -> Void) {
        self.preferences = preferences
        self.requestHide = requestHide
        normalSize = NSSize(width: preferences.windowWidth, height: preferences.windowHeight)
        let window = MainWindow(
            contentRect: NSRect(x: 0, y: 0, width: 1120, height: 780),
            styleMask: [.titled, .closable, .miniaturizable, .resizable],
            backing: .buffered,
            defer: false
        )
        super.init(window: window)
        window.title = "EdifierCtrl"
        window.identifier = NSUserInterfaceItemIdentifier("EdifierCtrl.main")
        window.isReleasedWhenClosed = false
        window.isRestorable = false
        window.tabbingMode = .disallowed
        window.collectionBehavior = [.fullScreenPrimary]
        window.delegate = self
        let hostingView = NSHostingView(rootView: RootView(model: model))
        hostingView.sizingOptions = []
        window.contentView = hostingView
        window.beforeZoom = { [weak self] in self?.willZoom() }
        window.afterZoom = { [weak self] in self?.didZoom() }
        restoreSize()
        restoring = false
    }

    required init?(coder: NSCoder) { return nil }

    func reveal() {
        guard let window else { return }
        if window.isMiniaturized { window.deminiaturize(nil) }
        window.makeKeyAndOrderFront(nil)
    }

    func hide() {
        persistState()
        window?.orderOut(nil)
    }

    func persistState() {
        guard let window, !restoring, !fullscreenTransition, !zoomTransition,
              !window.styleMask.contains(.fullScreen)
        else { return }
        resizeTask?.cancel()
        resizeTask = nil
        if !window.isZoomed {
            let contentSize = window.contentRect(forFrameRect: window.frame).size
            if contentSize.width > 0 && contentSize.height > 0 { normalSize = contentSize }
        }
        preferences.storeWindowState(width: normalSize.width, height: normalSize.height, zoomed: window.isZoomed)
    }

    func prepareForTermination() {
        // 全屏退出时保留进入全屏前的尺寸及 zoom 标志.
        persistState()
        resizeTask?.cancel()
        zoomTask?.cancel()
    }

    func windowShouldClose(_ sender: NSWindow) -> Bool {
        requestHide()
        return false
    }

    func windowDidEndLiveResize(_ notification: Notification) {
        persistState()
    }

    func windowDidResize(_ notification: Notification) {
        guard !restoring, !fullscreenTransition, !zoomTransition,
              let window, !window.inLiveResize, !window.styleMask.contains(.fullScreen)
        else { return }
        scheduleResizeSave()
    }

    func windowWillEnterFullScreen(_ notification: Notification) {
        persistState()
        fullscreenTransition = true
        resizeTask?.cancel()
    }

    func windowDidEnterFullScreen(_ notification: Notification) {
        fullscreenTransition = false
    }

    func windowWillExitFullScreen(_ notification: Notification) {
        fullscreenTransition = true
    }

    func windowDidExitFullScreen(_ notification: Notification) {
        fullscreenTransition = false
        scheduleResizeSave()
    }

    func windowDidFailToEnterFullScreen(_ window: NSWindow) {
        fullscreenTransition = false
        scheduleResizeSave()
    }

    func windowDidFailToExitFullScreen(_ window: NSWindow) {
        fullscreenTransition = false
    }

    private func restoreSize() {
        guard let window else { return }
        let screenFrame = (window.screen ?? NSScreen.main ?? NSScreen.screens.first)?.visibleFrame
            ?? NSRect(x: 0, y: 0, width: 1440, height: 900)
        let chrome = window.frameRect(forContentRect: NSRect(x: 0, y: 0, width: 1, height: 1)).height - 1
        let available = NSSize(width: max(1, screenFrame.width), height: max(1, screenFrame.height - chrome))
        let minimum = NSSize(width: min(900, available.width), height: min(650, available.height))
        window.contentMinSize = minimum
        normalSize.width = min(max(normalSize.width, minimum.width), available.width)
        normalSize.height = min(max(normalSize.height, minimum.height), available.height)
        window.setContentSize(normalSize)
        window.center()
        var frame = window.frame
        frame.origin.x = min(max(frame.origin.x, screenFrame.minX), screenFrame.maxX - frame.width)
        frame.origin.y = min(max(frame.origin.y, screenFrame.minY), screenFrame.maxY - frame.height)
        window.setFrame(frame, display: false)
        if preferences.windowZoomed { window.zoom(nil) }
    }

    private func willZoom() {
        guard !restoring, !fullscreenTransition, let window, !window.styleMask.contains(.fullScreen) else { return }
        if !window.isZoomed { normalSize = window.contentRect(forFrameRect: window.frame).size }
        zoomTransition = true
        resizeTask?.cancel()
    }

    private func didZoom() {
        guard !restoring, let window, !window.styleMask.contains(.fullScreen) else { return }
        // 先存普通尺寸和标志, 避免紧接着退出或进入全屏时丢失 zoom 状态.
        preferences.storeWindowState(width: normalSize.width, height: normalSize.height, zoomed: window.isZoomed)
        zoomTask?.cancel()
        zoomTask = Task { [weak self] in
            do { try await Task.sleep(nanoseconds: 250_000_000) }
            catch { return }
            guard let self, !Task.isCancelled else { return }
            self.zoomTransition = false
            self.persistState()
        }
    }

    private func scheduleResizeSave() {
        resizeTask?.cancel()
        resizeTask = Task { [weak self] in
            do { try await Task.sleep(nanoseconds: 200_000_000) }
            catch { return }
            guard !Task.isCancelled else { return }
            self?.persistState()
        }
    }
}

@MainActor
private final class MainWindow: NSWindow {
    var beforeZoom: (() -> Void)?
    var afterZoom: (() -> Void)?

    override func zoom(_ sender: Any?) {
        beforeZoom?()
        super.zoom(sender)
        afterZoom?()
    }
}
