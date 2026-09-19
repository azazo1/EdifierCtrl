import Foundation

/// Rust 动态库保留到进程退出, 回调同样使用进程级入口, 不持有会话对象.
enum NativeLogging {
    typealias Callback = @convention(c) (Int32, UnsafePointer<CChar>?, UnsafePointer<CChar>?, Int32) -> Void
    typealias Install = @convention(c) (Callback?) -> Int32
    typealias SetLevel = @convention(c) (Int32) -> Int32

    private static let lock = NSLock()
    private static var desiredLevel: Int32 = 3
    private static var updateLevel: SetLevel?

    static func install(_ install: Install, setLevel: @escaping SetLevel) throws {
        guard install(edifier_macos_log_event) == 0 else {
            throw NativeFailure(message: "无法接入核心日志, 请检查是否已有其他日志订阅器.")
        }
        lock.lock()
        updateLevel = setLevel
        let result = setLevel(desiredLevel)
        lock.unlock()
        guard result == 0 else { throw NativeFailure(message: "无法设置核心日志级别.") }
        AppLog.info("核心日志已接入应用日志.", category: "native")
    }

    static func setLevel(_ level: Int32) {
        lock.lock()
        desiredLevel = level
        let result = updateLevel?(level) ?? 0
        lock.unlock()
        if result != 0 { AppLog.error("切换核心日志级别失败.", category: "native") }
    }
}

@_cdecl("edifier_macos_log_event")
func edifier_macos_log_event(_ level: Int32, _ target: UnsafePointer<CChar>?,
                            _ message: UnsafePointer<CChar>?, _ flush: Int32) {
    guard let message else { return }
    // 在 C 回调返回前复制字符串. AppLog 可从 Rust 工作线程直接调用.
    AppLog.recordNative(level: level, target: target.map { String(cString: $0) } ?? "edifier_ffi",
                        message: String(cString: message))
    if flush != 0 { AppLog.flush() }
}
