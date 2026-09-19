import Foundation
import Security

/// 口令只进入 Keychain, service 使用数据目录摘要隔离正式, 调试和 fake 实例.
enum GroupSecret {
    private static var baseQuery: [String: Any] {
        [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: "EdifierCtrl.group." + AppPaths.instanceIdentifier,
            kSecAttrAccount as String: "group-passphrase",
            kSecAttrSynchronizable as String: false,
        ]
    }

    static func load() throws -> String? {
        var query = baseQuery
        query[kSecReturnData as String] = true
        query[kSecMatchLimit as String] = kSecMatchLimitOne
        var result: CFTypeRef?
        let status = SecItemCopyMatching(query as CFDictionary, &result)
        if status == errSecItemNotFound { return nil }
        try requireSuccess(status, operation: "读取组口令")
        guard let data = result as? Data, let secret = String(data: data, encoding: .utf8) else {
            throw SecretError.invalidData
        }
        return secret
    }

    static func save(_ secret: String) throws {
        if secret.isEmpty {
            try delete()
            return
        }
        let data = Data(secret.utf8)
        let status = SecItemUpdate(baseQuery as CFDictionary, [kSecValueData as String: data] as CFDictionary)
        if status == errSecItemNotFound {
            var query = baseQuery
            query[kSecValueData as String] = data
            query[kSecAttrAccessible as String] = kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly
            try requireSuccess(SecItemAdd(query as CFDictionary, nil), operation: "保存组口令")
        } else {
            try requireSuccess(status, operation: "更新组口令")
        }
        AppLog.info("组口令已保存至 Keychain.", category: "keychain")
    }

    static func delete() throws {
        let status = SecItemDelete(baseQuery as CFDictionary)
        guard status != errSecItemNotFound else { return }
        try requireSuccess(status, operation: "删除组口令")
        AppLog.info("已删除 Keychain 组口令.", category: "keychain")
    }

    private static func requireSuccess(_ status: OSStatus, operation: String) throws {
        guard status != errSecSuccess else { return }
        let error = SecretError.keychain(operation, status)
        AppLog.error(error.localizedDescription, category: "keychain")
        throw error
    }

    enum SecretError: LocalizedError {
        case keychain(String, OSStatus)
        case invalidData

        var errorDescription: String? {
            switch self {
            case let .keychain(operation, status):
                let reason = SecCopyErrorMessageString(status, nil) as String? ?? "未知 Keychain 错误"
                return "\(operation)失败: \(reason) (\(status))."
            case .invalidData:
                return "Keychain 中的组口令格式无效."
            }
        }
    }
}
