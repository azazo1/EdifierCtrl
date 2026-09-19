import CryptoKit
import Foundation

/// 配置, 组名, 日志和单实例标识始终使用同一数据目录.
enum AppPaths {
    static let dataDirectory: URL = {
        if let custom = environmentPath("EDIFIER_DATA_DIR") {
            return AppVersion.isFakeBuild ? custom.appendingPathComponent("fake", isDirectory: true) : custom
        }
        let support = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first
            ?? FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent("Library/Application Support", isDirectory: true)
        return support.appendingPathComponent(AppVersion.isFakeBuild ? "EdifierCtrl-fake" : "EdifierCtrl", isDirectory: true)
    }()

    static let logFile: URL = {
        guard let custom = environmentPath("EDIFIER_LOG_FILE") else {
            return dataDirectory.appendingPathComponent("logs/EdifierCtrl.log")
        }
        guard AppVersion.isFakeBuild else { return custom }
        let suffix = custom.pathExtension
        let stem = custom.deletingPathExtension().lastPathComponent
        return custom.deletingLastPathComponent().appendingPathComponent(stem + "-fake" + (suffix.isEmpty ? "" : "." + suffix))
    }()

    static let preferencesFile = dataDirectory.appendingPathComponent("preferences.json")

    static let instanceIdentifier: String = {
        let canonicalPath = dataDirectory.resolvingSymlinksInPath().standardizedFileURL.path
        return SHA256.hash(data: Data(canonicalPath.utf8)).map { String(format: "%02x", $0) }.joined()
    }()

    static func prepareDataDirectory() throws {
        try FileManager.default.createDirectory(
            at: dataDirectory,
            withIntermediateDirectories: true,
            attributes: [.posixPermissions: 0o700]
        )
    }

    private static func environmentPath(_ key: String) -> URL? {
        guard let value = ProcessInfo.processInfo.environment[key],
              !value.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        else { return nil }
        return URL(fileURLWithPath: (value as NSString).expandingTildeInPath).standardizedFileURL
    }
}
