import AppKit
import Darwin

/// Finder 的 reopen 与终端二次启动都复用已有窗口, 文件锁按数据目录隔离.
@MainActor
final class SingleInstanceController: NSObject {
    private var descriptor: Int32 = -1
    private var onActivation: (([String]) -> Void)?
    private let notificationName = Notification.Name("EdifierCtrl.activate." + AppPaths.instanceIdentifier)

    func acquire(onActivation: @escaping ([String]) -> Void) throws -> Bool {
        try AppPaths.prepareDataDirectory()
        let lockFile = AppPaths.dataDirectory.appendingPathComponent("instance.lock")
        descriptor = Darwin.open(lockFile.path, O_CREAT | O_RDWR | O_CLOEXEC, 0o600)
        guard descriptor >= 0 else { throw posixError() }
        guard flock(descriptor, LOCK_EX | LOCK_NB) == 0 else {
            let failure = errno
            Darwin.close(descriptor)
            descriptor = -1
            guard failure == EWOULDBLOCK || failure == EAGAIN else {
                throw POSIXError(POSIXErrorCode(rawValue: failure) ?? .EIO)
            }
            DistributedNotificationCenter.default().postNotificationName(
                notificationName,
                object: AppPaths.instanceIdentifier,
                userInfo: ["arguments": Array(CommandLine.arguments.dropFirst())],
                deliverImmediately: true
            )
            return false
        }
        self.onActivation = onActivation
        DistributedNotificationCenter.default().addObserver(
            self,
            selector: #selector(activationReceived(_:)),
            name: notificationName,
            object: AppPaths.instanceIdentifier,
            suspensionBehavior: .deliverImmediately
        )
        return true
    }

    func release() {
        DistributedNotificationCenter.default().removeObserver(self)
        onActivation = nil
        if descriptor >= 0 {
            flock(descriptor, LOCK_UN)
            Darwin.close(descriptor)
            descriptor = -1
        }
        // 不删除锁文件, 防止尚未退出的进程与新进程持有不同 inode.
    }

    @objc private func activationReceived(_ notification: Notification) {
        let arguments = notification.userInfo?["arguments"] as? [String] ?? []
        Task { @MainActor [weak self] in self?.onActivation?(arguments) }
    }

    private func posixError() -> POSIXError {
        POSIXError(POSIXErrorCode(rawValue: errno) ?? .EIO)
    }
}
