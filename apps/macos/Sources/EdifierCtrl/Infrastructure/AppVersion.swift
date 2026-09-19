import Foundation

/// 由打包步骤注入版本, 开发运行不读取仓库或修改版本号.
enum AppVersion {
    static let display: String = {
        guard let value = Bundle.main.object(forInfoDictionaryKey: "EdifierBuildVersion") as? String,
              !value.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        else { return "dev-build" }
        return value.trimmingCharacters(in: .whitespacesAndNewlines)
    }()

    static let isFakeBuild: Bool = {
        let value = Bundle.main.object(forInfoDictionaryKey: "EdifierFakeBuild")
        if let value = value as? NSNumber { return value.boolValue }
        if let value = value as? String {
            return ["true", "yes", "1"].contains(value.lowercased())
        }
        return false
    }()
}
