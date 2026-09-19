import AppKit

@main
@MainActor
struct EdifierCtrlApp {
    static func main() {
        let application = NSApplication.shared
        NSWindow.allowsAutomaticWindowTabbing = false
        application.setActivationPolicy(.accessory)
        let delegate = DesktopApplicationDelegate()
        application.delegate = delegate
        withExtendedLifetime(delegate) {
            application.run()
        }
    }
}
