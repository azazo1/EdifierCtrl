import AppKit
import Combine

@MainActor
enum DesktopCommand {
    case showWindow(String)
    case hideWindow
    case checkUpdates
    case toggleAutomaticUpdates
    case quit(String)
}

@MainActor
final class StatusItemController: NSObject, NSMenuDelegate {
    private let item: NSStatusItem
    private let model: AppModel
    private let preferences: AppPreferences
    private let send: (DesktopCommand) -> Void
    private let menuTrackingEnded: () -> Void
    private(set) var isTrackingMenu = false
    private let contextMenu = NSMenu()
    private var automaticItems: [NSMenuItem] = []
    private var connectionItem: NSMenuItem?
    private var subscriptions: Set<AnyCancellable> = []

    init(model: AppModel, preferences: AppPreferences, menuTrackingEnded: @escaping () -> Void, send: @escaping (DesktopCommand) -> Void) {
        self.model = model
        self.preferences = preferences
        self.menuTrackingEnded = menuTrackingEnded
        self.send = send
        item = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
        super.init()
        item.isVisible = true
        if let button = item.button {
            button.image = NSImage(systemSymbolName: "headphones", accessibilityDescription: "EdifierCtrl")
            button.image?.isTemplate = true
            button.target = self
            button.action = #selector(statusItemClicked(_:))
            button.sendAction(on: [.leftMouseUp, .rightMouseUp])
        }
        buildMenu(contextMenu, includeShow: true)
        contextMenu.delegate = self
        installApplicationMenu()
        model.$connectionLabel.combineLatest(model.$isConnected)
            .receive(on: RunLoop.main)
            .sink { [weak self] label, connected in
                self?.updateConnection(label: label, connected: connected)
            }
            .store(in: &subscriptions)
        preferences.$autoCheckUpdates
            .receive(on: RunLoop.main)
            .sink { [weak self] enabled in
                self?.automaticItems.forEach { $0.state = enabled ? .on : .off }
            }
            .store(in: &subscriptions)
        updateConnection(label: model.connectionLabel, connected: model.isConnected)
    }

    func remove() {
        subscriptions.removeAll()
        NSStatusBar.system.removeStatusItem(item)
    }

    func menuWillOpen(_ menu: NSMenu) {
        isTrackingMenu = true
        automaticItems.forEach { $0.state = preferences.autoCheckUpdates ? .on : .off }
    }

    func menuDidClose(_ menu: NSMenu) {
        isTrackingMenu = false
        menuTrackingEnded()
    }

    @objc private func statusItemClicked(_ sender: Any?) {
        guard let event = NSApp.currentEvent, let button = item.button else { return }
        if event.type == .rightMouseUp || event.modifierFlags.contains(.control) {
            AppLog.info("打开状态栏菜单.", category: "desktop")
            isTrackingMenu = true
            NSMenu.popUpContextMenu(contextMenu, with: event, for: button)
            isTrackingMenu = false
            menuTrackingEnded()
        } else {
            send(.showWindow("状态栏左键"))
        }
    }

    @objc private func showWindow(_ sender: Any?) { send(.showWindow("菜单")) }
    @objc private func checkUpdates(_ sender: Any?) { send(.checkUpdates) }
    @objc private func toggleAutomaticUpdates(_ sender: Any?) { send(.toggleAutomaticUpdates) }
    @objc private func quit(_ sender: Any?) { send(.quit("菜单")) }

    private func updateConnection(label: String, connected: Bool) {
        item.button?.toolTip = "EdifierCtrl\n\(label)"
        item.button?.appearsDisabled = !connected
        connectionItem?.title = label.isEmpty ? "未连接" : label
    }

    private func buildMenu(_ menu: NSMenu, includeShow: Bool) {
        menu.autoenablesItems = false
        let version = NSMenuItem(title: "EdifierCtrl \(AppVersion.display)", action: nil, keyEquivalent: "")
        version.isEnabled = false
        menu.addItem(version)
        if includeShow {
            let connection = NSMenuItem(title: model.connectionLabel, action: nil, keyEquivalent: "")
            connection.isEnabled = false
            menu.addItem(connection)
            connectionItem = connection
        }
        menu.addItem(.separator())
        menu.addItem(actionItem("显示主窗口", action: #selector(showWindow(_:))))
        menu.addItem(actionItem("检查更新...", action: #selector(checkUpdates(_:))))
        let automatic = actionItem("自动检查更新", action: #selector(toggleAutomaticUpdates(_:)))
        automatic.state = preferences.autoCheckUpdates ? .on : .off
        automaticItems.append(automatic)
        menu.addItem(automatic)
        menu.addItem(.separator())
        menu.addItem(actionItem("退出 EdifierCtrl", action: #selector(quit(_:)), key: "q"))
    }

    private func actionItem(_ title: String, action: Selector, key: String = "") -> NSMenuItem {
        let item = NSMenuItem(title: title, action: action, keyEquivalent: key)
        item.target = self
        return item
    }

    private func installApplicationMenu() {
        let menu = NSMenu()
        let applicationItem = NSMenuItem(title: "EdifierCtrl", action: nil, keyEquivalent: "")
        let applicationMenu = NSMenu(title: "EdifierCtrl")
        applicationMenu.delegate = self
        buildMenu(applicationMenu, includeShow: false)
        applicationItem.submenu = applicationMenu
        menu.addItem(applicationItem)

        let editItem = NSMenuItem(title: "编辑", action: nil, keyEquivalent: "")
        let editMenu = NSMenu(title: "编辑")
        editMenu.addItem(withTitle: "撤销", action: Selector(("undo:")), keyEquivalent: "z")
        editMenu.addItem(withTitle: "重做", action: Selector(("redo:")), keyEquivalent: "Z")
        editMenu.addItem(.separator())
        editMenu.addItem(withTitle: "剪切", action: #selector(NSText.cut(_:)), keyEquivalent: "x")
        editMenu.addItem(withTitle: "复制", action: #selector(NSText.copy(_:)), keyEquivalent: "c")
        editMenu.addItem(withTitle: "粘贴", action: #selector(NSText.paste(_:)), keyEquivalent: "v")
        editMenu.addItem(withTitle: "全选", action: #selector(NSText.selectAll(_:)), keyEquivalent: "a")
        editItem.submenu = editMenu
        menu.addItem(editItem)

        let windowItem = NSMenuItem(title: "窗口", action: nil, keyEquivalent: "")
        let windowMenu = NSMenu(title: "窗口")
        windowMenu.addItem(withTitle: "最小化", action: #selector(NSWindow.performMiniaturize(_:)), keyEquivalent: "m")
        windowMenu.addItem(withTitle: "缩放", action: #selector(NSWindow.performZoom(_:)), keyEquivalent: "")
        windowMenu.addItem(withTitle: "关闭窗口", action: #selector(NSWindow.performClose(_:)), keyEquivalent: "w")
        windowItem.submenu = windowMenu
        menu.addItem(windowItem)
        NSApp.mainMenu = menu
        NSApp.windowsMenu = windowMenu
    }
}
