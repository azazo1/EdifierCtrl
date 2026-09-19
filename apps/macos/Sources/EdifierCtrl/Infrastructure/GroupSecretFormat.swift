import Foundation

struct GroupSecretDocument: Codable {
    var version = 1
    let passphrase: String
}

/// 独立管理口令文件格式, 未知版本与无效文件均保留原文供用户处理.
enum GroupSecretFormat {
    static let currentVersion = 1

    private struct Header: Decodable {
        let version: Int
    }

    static func decode(_ data: Data) throws -> GroupSecretDocument {
        do {
            let decoder = JSONDecoder()
            let header = try decoder.decode(Header.self, from: data)
            guard header.version == currentVersion else { throw FormatError.unsupportedVersion(header.version) }
            return try decoder.decode(GroupSecretDocument.self, from: data)
        } catch let error as FormatError {
            throw error
        } catch {
            // 解码器的原始描述可能包含口令文本, 只向 UI 和日志传递固定错误.
            throw FormatError.invalidData
        }
    }

    enum FormatError: LocalizedError {
        case unsupportedVersion(Int)
        case invalidData

        var errorDescription: String? {
            switch self {
            case let .unsupportedVersion(version): return "不支持组口令文件版本 \(version), 原文件已保留."
            case .invalidData: return "数据目录中的组口令文件格式无效, 原文件已保留."
            }
        }
    }
}
