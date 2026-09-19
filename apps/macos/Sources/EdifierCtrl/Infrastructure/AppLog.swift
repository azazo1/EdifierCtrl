import Darwin
import Foundation
import os

/// 系统日志用于系统诊断, 串行文件日志用于可控留存和退出刷盘.
enum AppLog {
    private static let writer = LogWriter()

    static func info(_ message: String, category: String = "app") {
        writer.record(message, level: .info, category: category)
    }

    static func debug(_ message: String, category: String = "app") {
        writer.record(message, level: .debug, category: category)
    }

    static func error(_ message: String, category: String = "app") {
        writer.record(message, level: .error, category: category)
    }

    static func setVerbose(_ enabled: Bool) {
        writer.setVerbose(enabled)
        NativeLogging.setLevel(writer.nativeLevel)
    }

    static func recordNative(level: Int32, target: String, message: String) {
        let mapped: LogWriter.Level
        switch level {
        case 1: mapped = .error
        case 2: mapped = .warning
        case 4: mapped = .debug
        case 5: mapped = .trace
        default: mapped = .info
        }
        // Rust 已按缺省级别及 RUST_LOG 指令过滤, 此处保留 target 的显式覆盖.
        writer.record(message, level: mapped, category: target, filtered: true)
    }

    static func flush() {
        writer.flush()
    }

    static func installExceptionHandler() {
        NSSetUncaughtExceptionHandler { exception in
            AppLog.error("未捕获异常: \(exception.name.rawValue), \(exception.reason ?? "无原因").\n\(exception.callStackSymbols.joined(separator: "\n"))")
            AppLog.flush()
        }
    }
}

private final class LogWriter: @unchecked Sendable {
    enum Level: String {
        case trace = "TRACE"
        case debug = "DEBUG"
        case info = "INFO"
        case warning = "WARN"
        case error = "ERROR"
    }

    private let queue = DispatchQueue(label: "EdifierCtrl.log-writer", qos: .utility)
    private let queueKey = DispatchSpecificKey<Bool>()
    private let settingsLock = NSLock()
    private var verbose = false
    private let forcedLevel = ProcessInfo.processInfo.environment["EDIFIER_LOG_LEVEL"]?.lowercased()
    private let systemLog = Logger(subsystem: Bundle.main.bundleIdentifier ?? "EdifierCtrl", category: "application")
    private let diagnostics = Logger(subsystem: Bundle.main.bundleIdentifier ?? "EdifierCtrl", category: "log-storage")
    private var standardErrorFailed = false
    private let timestamp = ISO8601DateFormatter()
    private let dayFormatter: DateFormatter = {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "en_US_POSIX")
        formatter.dateFormat = "yyyy-MM-dd"
        return formatter
    }()
    private let maximumBytes: UInt64 = 5 * 1024 * 1024
    private let retainedArchives = 10
    private var handle: FileHandle?
    private var currentBytes: UInt64 = 0
    private var currentDay = ""
    private var lastFailure: String?

    init() {
        timestamp.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        queue.setSpecific(key: queueKey, value: true)
    }

    func setVerbose(_ enabled: Bool) {
        settingsLock.lock()
        verbose = enabled
        settingsLock.unlock()
    }

    var nativeLevel: Int32 {
        settingsLock.lock()
        defer { settingsLock.unlock() }
        if verbose || forcedLevel == "trace" { return 5 }
        return forcedLevel == "debug" ? 4 : 3
    }

    func record(_ message: String, level: Level, category: String, filtered: Bool = false) {
        let threshold = nativeLevel
        let shouldWrite = level == .trace ? threshold >= 5 : level == .debug ? threshold >= 4 : true
        guard filtered || shouldWrite else { return }
        let output = "[\(category)] \(message)"
        switch level {
        case .trace, .debug: systemLog.debug("\(output, privacy: .public)")
        case .info: systemLog.info("\(output, privacy: .public)")
        case .warning: systemLog.warning("\(output, privacy: .public)")
        case .error: systemLog.error("\(output, privacy: .public)")
        }
        let date = Date()
        queue.async { [self] in
            let line = "\(timestamp.string(from: date)) \(level.rawValue) \(output)\n"
            let data = Data(line.utf8)
            writeStandardError(data)
            do {
                try openIfNeeded()
                let day = dayFormatter.string(from: date)
                if currentBytes > 0 && (currentDay != day || currentBytes + UInt64(data.count) > maximumBytes) {
                    try rotate()
                    try openIfNeeded()
                }
                try handle?.write(contentsOf: data)
                currentBytes += UInt64(data.count)
                currentDay = day
                lastFailure = nil
            } catch {
                reportStorageFailure(error)
                try? handle?.close()
                handle = nil
            }
        }
    }

    func flush() {
        let operation = { [self] in
            do { try handle?.synchronize() }
            catch { reportStorageFailure(error) }
        }
        if DispatchQueue.getSpecific(key: queueKey) == true { operation() }
        else { queue.sync(execute: operation) }
    }

    private func openIfNeeded() throws {
        guard handle == nil else { return }
        let file = AppPaths.logFile
        let manager = FileManager.default
        try manager.createDirectory(at: file.deletingLastPathComponent(), withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
        if !manager.fileExists(atPath: file.path) {
            guard manager.createFile(atPath: file.path, contents: nil, attributes: [.posixPermissions: 0o600]) else {
                throw CocoaError(.fileWriteUnknown)
            }
        }
        let attributes = try manager.attributesOfItem(atPath: file.path)
        currentDay = dayFormatter.string(from: attributes[.modificationDate] as? Date ?? Date())
        let newHandle = try FileHandle(forWritingTo: file)
        do { currentBytes = try newHandle.seekToEnd() }
        catch {
            try? newHandle.close()
            throw error
        }
        handle = newHandle
        pruneArchivesIfPossible()
    }

    private func rotate() throws {
        try handle?.synchronize()
        try handle?.close()
        handle = nil
        let source = AppPaths.logFile
        let archiveName = source.lastPathComponent + "." + currentDay + "." + UUID().uuidString + ".archive"
        let destination = source.deletingLastPathComponent().appendingPathComponent(archiveName)
        try FileManager.default.moveItem(at: source, to: destination)
        currentBytes = 0
        pruneArchivesIfPossible()
    }

    private func pruneArchivesIfPossible() {
        do { try pruneArchives() }
        catch {
            diagnostics.error("旧日志清理失败, 继续写入主日志: \(error.localizedDescription, privacy: .public)")
        }
    }

    private func pruneArchives() throws {
        let source = AppPaths.logFile
        let keys: Set<URLResourceKey> = [.contentModificationDateKey, .isRegularFileKey]
        let files = try FileManager.default.contentsOfDirectory(at: source.deletingLastPathComponent(), includingPropertiesForKeys: Array(keys))
        let prefix = source.lastPathComponent + "."
        let archives = try files.filter { file in
            guard file.lastPathComponent.hasPrefix(prefix), file.pathExtension == "archive" else { return false }
            return try file.resourceValues(forKeys: keys).isRegularFile == true
        }.map { file in
            (file, try file.resourceValues(forKeys: keys).contentModificationDate ?? .distantPast)
        }.sorted { $0.1 > $1.1 }
        for (file, _) in archives.dropFirst(retainedArchives) {
            try FileManager.default.removeItem(at: file)
        }
    }

    private func reportStorageFailure(_ error: Error) {
        let text = error.localizedDescription
        guard lastFailure != text else { return }
        lastFailure = text
        diagnostics.error("文件日志失败, 系统日志继续可用: \(text, privacy: .public)")
        writeStandardError(Data("EdifierCtrl 文件日志失败: \(text)\n".utf8))
    }

    private func writeStandardError(_ data: Data) {
        do {
            try FileHandle.standardError.write(contentsOf: data)
            standardErrorFailed = false
        } catch {
            guard !standardErrorFailed else { return }
            standardErrorFailed = true
            diagnostics.error("标准错误日志写入失败: \(error.localizedDescription, privacy: .public)")
        }
    }
}
