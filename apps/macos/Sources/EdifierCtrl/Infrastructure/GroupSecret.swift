import Darwin
import Foundation

/// 组口令随数据目录隔离, 只允许当前用户读取, 不访问钥匙串.
enum GroupSecret {
    static func load(from directory: URL = AppPaths.dataDirectory) throws -> String? {
        let data: Data
        do {
            data = try Data(contentsOf: file(in: directory))
        } catch CocoaError.fileReadNoSuchFile {
            return nil
        }
        return try GroupSecretFormat.decode(data).passphrase
    }

    static func save(_ secret: String, in directory: URL = AppPaths.dataDirectory) throws {
        if secret.isEmpty {
            try delete(in: directory)
            return
        }
        // 无法读取的格式必须保留, 不能用当前版本静默覆盖.
        _ = try load(from: directory)
        let manager = FileManager.default
        try manager.createDirectory(at: directory, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
        let data = try JSONEncoder().encode(GroupSecretDocument(passphrase: secret))
        let temporary = directory.appendingPathComponent(".group-secret-\(UUID().uuidString).tmp")
        guard manager.createFile(atPath: temporary.path, contents: data, attributes: [.posixPermissions: 0o600]) else {
            throw CocoaError(.fileWriteUnknown)
        }
        defer { try? manager.removeItem(at: temporary) }
        let handle = try FileHandle(forWritingTo: temporary)
        defer { try? handle.close() }
        try handle.synchronize()
        // 同目录 rename 原子替换, 保持新文件的 0600 权限.
        let status = temporary.withUnsafeFileSystemRepresentation { source in
            file(in: directory).withUnsafeFileSystemRepresentation { destination in
                rename(source!, destination!)
            }
        }
        guard status == 0 else { throw NSError(domain: NSPOSIXErrorDomain, code: Int(errno)) }
        AppLog.info("组口令已保存至当前数据目录.", category: "group-secret")
    }

    static func delete(in directory: URL = AppPaths.dataDirectory) throws {
        do {
            try FileManager.default.removeItem(at: file(in: directory))
        } catch CocoaError.fileNoSuchFile {
            return
        }
        AppLog.info("已删除当前数据目录中的组口令.", category: "group-secret")
    }

    static func file(in directory: URL) -> URL {
        directory.appendingPathComponent("group-secret.json")
    }
}
