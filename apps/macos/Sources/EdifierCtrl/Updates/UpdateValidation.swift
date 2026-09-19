import Foundation

struct UpdateFailure: LocalizedError, Sendable {
    let message: String
    var errorDescription: String? { message }
    init(_ message: String) { self.message = message }
}

struct UpdateVersion: Comparable, Sendable {
    let components: [Int]
    let prerelease: [String]

    init?(_ display: String) {
        var version = display.hasPrefix("v") ? String(display.dropFirst()) : display
        if let suffix = version.range(of: #"[\-\^][0-9a-fA-F]{7,40}$"#, options: .regularExpression) {
            version.removeSubrange(suffix)
        }
        guard version.range(of: #"^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?(\+[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?$"#, options: .regularExpression) != nil else { return nil }
        let withoutMetadata = String(version.split(separator: "+", maxSplits: 1)[0])
        let pieces = withoutMetadata.split(separator: "-", maxSplits: 1)
        let numbers = pieces[0].split(separator: ".").compactMap { Int($0) }
        guard numbers.count == 3 else { return nil }
        components = numbers
        prerelease = pieces.count == 2 ? pieces[1].split(separator: ".").map(String.init) : []
        guard !prerelease.contains(where: { $0.allSatisfy(\.isNumber) && $0.count > 1 && $0.hasPrefix("0") }) else { return nil }
    }

    static func < (lhs: Self, rhs: Self) -> Bool {
        if lhs.components != rhs.components {
            return lhs.components.lexicographicallyPrecedes(rhs.components)
        }
        if lhs.prerelease.isEmpty { return false }
        if rhs.prerelease.isEmpty { return true }
        for (left, right) in zip(lhs.prerelease, rhs.prerelease) where left != right {
            let leftNumeric = left.allSatisfy(\.isNumber)
            let rightNumeric = right.allSatisfy(\.isNumber)
            if leftNumeric && rightNumeric {
                return left.count == right.count ? left < right : left.count < right.count
            }
            if leftNumeric != rightNumeric { return leftNumeric }
            return left < right
        }
        return lhs.prerelease.count < rhs.prerelease.count
    }
}

struct UpdateRelease: Decodable, Sendable {
    struct Asset: Decodable, Sendable {
        let name: String
        let size: Int64
        let browserDownloadURL: URL
        enum CodingKeys: String, CodingKey {
            case name, size
            case browserDownloadURL = "browser_download_url"
        }
    }
    let tagName: String
    let body: String?
    let htmlURL: URL
    let draft: Bool
    let prerelease: Bool
    let assets: [Asset]
    enum CodingKeys: String, CodingKey {
        case body, draft, prerelease, assets
        case tagName = "tag_name"
        case htmlURL = "html_url"
    }
}

struct UpdateCandidate: Sendable {
    let release: UpdateRelease
    let archive: UpdateRelease.Asset
    let checksum: UpdateRelease.Asset
    let digest: String
}

enum UpdateValidation {
    static var architecture: String {
        #if arch(arm64)
        return "aarch64"
        #else
        return "x86_64"
        #endif
    }

    static func repository(_ value: String?) throws -> String {
        guard let value, value.range(of: #"^[A-Za-z0-9][A-Za-z0-9_.-]*/[A-Za-z0-9][A-Za-z0-9_.-]*$"#, options: .regularExpression) != nil,
              !value.split(separator: "/").contains(where: { $0 == "." || $0 == ".." }) else {
            throw UpdateFailure("此构建尚未配置 GitHub Release 仓库. 请使用已配置仓库的发行包.")
        }
        return value
    }

    static func assetName(tag: String, architecture: String = architecture) throws -> String {
        guard UpdateVersion(tag) != nil,
              tag.range(of: #"^[A-Za-z0-9.+-]+$"#, options: .regularExpression) != nil,
              ["aarch64", "x86_64"].contains(architecture) else {
            throw UpdateFailure("Release 版本或架构格式无效.")
        }
        return "EdifierCtrl-\(tag)-macos-\(architecture).dmg"
    }

    static func checksum(_ text: String, named name: String) throws -> String {
        var matches: [String] = []
        // Swift 将 CRLF 视为一个 Character, 使用换行属性同时处理 LF 和 CRLF.
        for line in text.split(whereSeparator: \.isNewline) {
            guard line.count >= 66 else { continue }
            let digest = String(line.prefix(64))
            let rest = line.dropFirst(64)
            guard rest.hasPrefix("  ") || rest.hasPrefix(" *") else { continue }
            guard String(rest.dropFirst(2)) == name else { continue }
            guard digest.range(of: #"^[0-9a-fA-F]{64}$"#, options: .regularExpression) != nil else {
                throw UpdateFailure("SHA256SUMS 中的摘要无效.")
            }
            matches.append(digest.lowercased())
        }
        guard matches.count == 1 else {
            throw UpdateFailure("SHA256SUMS 必须包含唯一的目标安装包校验行.")
        }
        return matches[0]
    }

    static func assetURL(_ url: URL, repository: String, tag: String, name: String) throws {
        guard url.scheme == "https", url.host == "github.com", url.port == nil,
              url.user == nil, url.password == nil, url.query == nil, url.fragment == nil,
              url.path == "/\(repository)/releases/download/\(tag)/\(name)" else {
            throw UpdateFailure("Release 资产地址与预期仓库或文件名不一致.")
        }
    }

    static func releaseURL(_ url: URL, repository: String, tag: String) throws {
        guard url.scheme == "https", url.host == "github.com", url.port == nil,
              url.user == nil, url.password == nil, url.query == nil, url.fragment == nil,
              url.path == "/\(repository)/releases/tag/\(tag)" else {
            throw UpdateFailure("Release 页面地址无效.")
        }
    }
}

struct UpdateInstallPaths: Equatable, Sendable {
    let bundle: URL
    let staging: URL
    let backup: URL

    init(bundle: URL, token: String) throws {
        guard bundle.isFileURL, bundle.pathExtension == "app", bundle.path.hasPrefix("/"),
              bundle.standardizedFileURL == bundle,
              bundle.deletingLastPathComponent().path != "/",
              token.range(of: #"^[A-Za-z0-9-]+$"#, options: .regularExpression) != nil else {
            throw UpdateFailure("更新目标必须是具有有效父目录的 .app bundle.")
        }
        self.bundle = bundle
        let parent = bundle.deletingLastPathComponent()
        staging = parent.appendingPathComponent(".\(bundle.lastPathComponent).update-\(token)")
        backup = parent.appendingPathComponent("\(bundle.lastPathComponent).old")
        guard staging.deletingLastPathComponent() == parent, backup.deletingLastPathComponent() == parent else {
            throw UpdateFailure("更新暂存与备份路径必须与应用位于同一目录.")
        }
    }
}
