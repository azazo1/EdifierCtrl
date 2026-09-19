import AppKit
import Foundation

extension Notification.Name {
    static let edifierQuitRequested = Notification.Name("EdifierCtrl.quitRequested")
}

enum UpdateInstaller {
    static func installedBundle(executable: URL? = Bundle.main.executableURL, bundle: URL = Bundle.main.bundleURL) -> URL? {
        guard let executable, bundle.pathExtension == "app",
              executable.deletingLastPathComponent() == bundle.appendingPathComponent("Contents/MacOS"),
              bundle.resolvingSymlinksInPath() == bundle.standardizedFileURL,
              !bundle.path.hasPrefix("/Volumes/"), !bundle.path.contains("/AppTranslocation/") else { return nil }
        return bundle
    }

    // 文件路径作为 Process 参数传入, 不拼接到 shell 程序中.
    static func handOff(archive: URL, digest: String, version: String, directory: URL, bundle: URL, dataDirectory: URL, logFile: URL) throws {
        let paths = try UpdateInstallPaths(bundle: bundle, token: UUID().uuidString)
        guard let resource = Bundle.main.url(forResource: "apply-update", withExtension: "sh") else {
            throw UpdateFailure("此应用包缺少更新安装助手. 请打开安装包手动安装.")
        }
        let script = directory.appendingPathComponent("apply-update.sh")
        if try UpdateFiles.regularFile(script) { try FileManager.default.removeItem(at: script) }
        try FileManager.default.copyItem(at: resource, to: script)
        try FileManager.default.setAttributes([.posixPermissions: 0o700], ofItemAtPath: script.path)
        let log = directory.appendingPathComponent("apply-update.log")
        if !(try UpdateFiles.regularFile(log)) {
            guard FileManager.default.createFile(atPath: log.path, contents: nil, attributes: [.posixPermissions: 0o600]) else {
                throw UpdateFailure("无法创建安装日志.")
            }
        }
        let launcher = Process()
        launcher.executableURL = URL(fileURLWithPath: "/bin/bash")
        launcher.arguments = ["-c", "(/usr/bin/nohup /bin/bash \"$@\" </dev/null &)", "edifier-update", script.path,
                              String(ProcessInfo.processInfo.processIdentifier), paths.bundle.path, paths.staging.path,
                              paths.backup.path, archive.path, digest, version, directory.path,
                              Bundle.main.bundleIdentifier ?? "com.edifierctrl.mac", dataDirectory.path, logFile.path]
        // helper 会立即把全部输出转到固定日志, launcher 输出保留在同一日志.
        let output = try FileHandle(forWritingTo: log)
        try output.seekToEnd()
        launcher.standardOutput = output
        launcher.standardError = output
        launcher.standardInput = FileHandle.nullDevice
        try launcher.run()
        launcher.waitUntilExit()
        try output.close()
        guard launcher.terminationStatus == 0 else { throw UpdateFailure("无法启动独立更新助手.") }
    }

    static func previousFailure(directory: URL) throws -> String? {
        let result = directory.appendingPathComponent("apply-update-result.txt")
        guard try UpdateFiles.regularFile(result) else { return nil }
        let values = try result.resourceValues(forKeys: [.fileSizeKey])
        guard (values.fileSize ?? 0) <= 64 * 1024 else { throw UpdateFailure("更新结果文件过大.") }
        let message = try String(contentsOf: result, encoding: .utf8)
        try FileManager.default.removeItem(at: result)
        return message
    }

    static func cleanPreviousArtifacts(directory: URL) -> Bool {
        let manager = FileManager.default
        let pidFile = directory.appendingPathComponent("apply-update.pid")
        if (try? UpdateFiles.regularFile(pidFile)) == true,
           let text = try? String(contentsOf: pidFile, encoding: .utf8),
           let pid = Int32(text.trimmingCharacters(in: .whitespacesAndNewlines)),
           pid > 1, kill(pid, 0) == 0 { return false }
        if (try? UpdateFiles.regularFile(pidFile)) == true { try? manager.removeItem(at: pidFile) }
        let script = directory.appendingPathComponent("apply-update.sh")
        if (try? UpdateFiles.regularFile(script)) == true { try? manager.removeItem(at: script) }
        guard let entries = try? manager.contentsOfDirectory(at: directory, includingPropertiesForKeys: [.isDirectoryKey, .isSymbolicLinkKey]) else { return true }
        for entry in entries where entry.lastPathComponent.range(of: #"^mount-[A-Za-z0-9]+$"#, options: .regularExpression) != nil {
            guard let values = try? entry.resourceValues(forKeys: [.isDirectoryKey, .isSymbolicLinkKey]), values.isDirectory == true, values.isSymbolicLink != true else { continue }
            let detach = Process()
            detach.executableURL = URL(fileURLWithPath: "/usr/bin/hdiutil")
            detach.arguments = ["detach", entry.path]
            let log = directory.appendingPathComponent("apply-update.log")
            guard (try? UpdateFiles.regularFile(log)) == true, let output = try? FileHandle(forWritingTo: log) else { continue }
            defer { try? output.close() }
            _ = try? output.seekToEnd()
            detach.standardOutput = output
            detach.standardError = output
            do {
                try detach.run()
                detach.waitUntilExit()
                // 只删除已卸载的空挂载点, 绝不递归删除挂载内容.
                _ = rmdir(entry.path)
            } catch { Task { @MainActor in AppLog.debug("更新挂载点清理暂缓: \(error.localizedDescription)") } }
        }
        return true
    }

    // 仅清理严格由当前 bundle 推导出的旧备份. 挂载点由 helper 的 EXIT trap 卸载.
    static func cleanPreviousBackup(bundle: URL?) {
        guard let bundle, let paths = try? UpdateInstallPaths(bundle: bundle, token: "cleanup"),
              let current = Bundle(url: bundle), let previous = Bundle(url: paths.backup),
              current.bundleIdentifier == previous.bundleIdentifier,
              (try? paths.backup.resourceValues(forKeys: [.isSymbolicLinkKey]).isSymbolicLink) == false else { return }
        do { try FileManager.default.removeItem(at: paths.backup) }
        catch { Task { @MainActor in AppLog.debug("更新备份清理暂缓: \(error.localizedDescription)") } }
    }
}
