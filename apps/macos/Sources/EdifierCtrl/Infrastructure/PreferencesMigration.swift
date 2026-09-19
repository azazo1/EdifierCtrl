import Foundation

struct PreferencesDocument: Codable {
    var version = 1
    var installationID = UUID().uuidString
    var startHidden = false
    var hideDockWhenHidden = true
    var autoJoinGroup = false
    var verboseLogging = false
    var selectedProfile = "basedevice"
    var lastDeviceAddress = ""
    var lastDeviceName = ""
    var windowWidth = 1120.0
    var windowHeight = 780.0
    var windowZoomed = false
    var autoCheckUpdates = true
    var skippedUpdateVersion = ""

    init() {}

    enum CodingKeys: String, CodingKey {
        case version, installationID, startHidden, hideDockWhenHidden, autoJoinGroup, verboseLogging
        case selectedProfile, lastDeviceAddress, lastDeviceName, windowWidth, windowHeight, windowZoomed
        case autoCheckUpdates, skippedUpdateVersion
    }

    init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        version = try values.decode(Int.self, forKey: .version)
        installationID = try values.decodeIfPresent(String.self, forKey: .installationID) ?? UUID().uuidString
        startHidden = try values.decodeIfPresent(Bool.self, forKey: .startHidden) ?? false
        hideDockWhenHidden = try values.decodeIfPresent(Bool.self, forKey: .hideDockWhenHidden) ?? true
        autoJoinGroup = try values.decodeIfPresent(Bool.self, forKey: .autoJoinGroup) ?? false
        verboseLogging = try values.decodeIfPresent(Bool.self, forKey: .verboseLogging) ?? false
        selectedProfile = try values.decodeIfPresent(String.self, forKey: .selectedProfile) ?? "basedevice"
        lastDeviceAddress = try values.decodeIfPresent(String.self, forKey: .lastDeviceAddress) ?? ""
        lastDeviceName = try values.decodeIfPresent(String.self, forKey: .lastDeviceName) ?? ""
        windowWidth = try values.decodeIfPresent(Double.self, forKey: .windowWidth) ?? 1120
        windowHeight = try values.decodeIfPresent(Double.self, forKey: .windowHeight) ?? 780
        windowZoomed = try values.decodeIfPresent(Bool.self, forKey: .windowZoomed) ?? false
        autoCheckUpdates = try values.decodeIfPresent(Bool.self, forKey: .autoCheckUpdates) ?? true
        skippedUpdateVersion = try values.decodeIfPresent(String.self, forKey: .skippedUpdateVersion) ?? ""
    }
}

/// 每次格式演进在此明确处理源版本和目标版本, 不在设置对象中隐式迁移.
enum PreferencesMigration {
    static let currentVersion = 1

    private struct Header: Decodable {
        let version: Int
    }

    static func decode(_ data: Data) throws -> PreferencesDocument {
        let decoder = JSONDecoder()
        let header = try decoder.decode(Header.self, from: data)
        switch header.version {
        case currentVersion:
            var document = try decoder.decode(PreferencesDocument.self, from: data)
            if UUID(uuidString: document.installationID) == nil {
                document.installationID = UUID().uuidString
                AppLog.info("设置中的安装标识无效, 已生成新的标识.", category: "preferences")
            }
            if !document.windowWidth.isFinite || document.windowWidth < 1 { document.windowWidth = 1120 }
            if !document.windowHeight.isFinite || document.windowHeight < 1 { document.windowHeight = 780 }
            if document.selectedProfile.isEmpty { document.selectedProfile = "basedevice" }
            return document
        default:
            throw MigrationError.unsupportedVersion(header.version)
        }
    }

    enum MigrationError: LocalizedError {
        case unsupportedVersion(Int)

        var errorDescription: String? {
            switch self {
            case let .unsupportedVersion(version):
                return "不支持设置版本 \(version), 当前支持版本 \(PreferencesMigration.currentVersion). 原文件已保留."
            }
        }
    }
}
