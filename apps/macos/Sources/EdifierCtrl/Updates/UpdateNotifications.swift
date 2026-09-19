import Foundation
import UserNotifications

extension Notification.Name {
    static let edifierShowWindowRequested = Notification.Name("EdifierCtrl.showWindowRequested")
}

final class UpdateNotificationDelegate: NSObject, UNUserNotificationCenterDelegate, @unchecked Sendable {
    func userNotificationCenter(_ center: UNUserNotificationCenter, didReceive response: UNNotificationResponse, withCompletionHandler completionHandler: @escaping () -> Void) {
        if response.notification.request.identifier == "EdifierCtrl.update" {
            Task { @MainActor in
                UpdateManager.shared.isPresented = true
                NotificationCenter.default.post(name: .edifierShowWindowRequested, object: nil)
            }
        }
        completionHandler()
    }

    func userNotificationCenter(_ center: UNUserNotificationCenter, willPresent notification: UNNotification, withCompletionHandler completionHandler: @escaping (UNNotificationPresentationOptions) -> Void) {
        completionHandler([.banner, .list])
    }
}
