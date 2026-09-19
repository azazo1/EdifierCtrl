import Foundation

struct HeadphoneDevice: Decodable, Identifiable, Equatable {
    let address: String
    let name: String
    let kind: String
    var serviceUuid: String?
    var id: String { address }
    var displayName: String { name.isEmpty ? "未命名耳机" : name }
    var isEdifier: Bool {
        let lower = name.lowercased()
        return lower.contains("edifier") || lower.contains("漫步者") || lower.hasPrefix("w820") || lower.hasPrefix("w200")
    }
}

struct HeadphoneProfile: Decodable, Identifiable, Equatable {
    let id: String
    let displayName: String
    let maxNameLen: Int
    let uniqueServiceUuid: String?
    let features: [String]
    func supports(_ feature: String) -> Bool { features.contains(feature) }
}

struct GroupPeer: Decodable, Identifiable, Equatable {
    let id: String
    let hostname: String
    let os: String
    let canAudio: Bool
    let holding: String?
    let appVersion: String
    var displayName: String { hostname.isEmpty ? id : hostname }
    var platformName: String {
        switch os.lowercased() {
        case "macos", "mac": return "macOS"
        case "android": return "Android"
        case "windows": return "Windows"
        case "linux": return "Linux"
        default: return os
        }
    }
    var symbol: String { os.lowercased() == "android" ? "smartphone" : "desktopcomputer" }
}

struct HeadsetNotification: Decodable {
    let kind: String
    var percent: Int?
    var mode: String?
    var ambientVolume: Int?
    var effect: String?
    var on: Bool?
    var volume: Int?
    var minutes: Int?
    var address: String?
    var version: String?
    var name: String?
    var normal: Bool?
    var reduction: Bool?
    var ambient: Bool?
}

struct HandoffProgress: Decodable, Equatable {
    let kind: String
    var reason: String?
    var isActive: Bool { ["requesting", "waiting_peer", "releasing", "connecting", "fallback_cd"].contains(kind) }
    var title: String {
        switch kind {
        case "requesting": return "正在请求交接"
        case "waiting_peer": return "等待另一台设备释放耳机"
        case "releasing": return "正在将耳机交给另一台设备"
        case "connecting": return "正在连接本机音频"
        case "fallback_cd": return "正在尝试释放原连接"
        case "done": return "交接已完成"
        case "failed": return "交接未完成"
        case "busy": return "已有交接正在进行"
        default: return "准备就绪"
        }
    }
    var step: Int {
        switch kind {
        case "requesting": return 0
        case "waiting_peer", "releasing", "fallback_cd": return 1
        case "connecting": return 2
        case "done": return 3
        default: return 0
        }
    }
}

struct NativeEvent: Decodable {
    let kind: String
    var connected: Bool?
    var address: String?
    var notification: HeadsetNotification?
    var state: String?
    var peer: GroupPeer?
    var progress: HandoffProgress?
    var text: String?
}

struct SessionState: Equatable {
    var connected = false
    var address: String?
    var name: String?
    var battery: Int?
    var firmware: String?
    var mac: String?
    var noise: String?
    var ambientVolume: Int?
    var effect: String?
    var gameMode: Bool?
    var ldac: String?
    var promptVolume: Int?
    var shutdownEnabled: Bool?
    var shutdownMinutes: Int?
    var autoPowerOff: Bool?
    var controlNormal: Bool?
    var controlReduction: Bool?
    var controlAmbient: Bool?
    var audio = "unknown"
    var holding: String?
    var handoff: HandoffProgress?

    mutating func apply(_ event: NativeEvent) {
        switch event.kind {
        case "bt_state":
            if event.connected == true {
                if let next = event.address, BluetoothAddress.normalize(next) != address.flatMap(BluetoothAddress.normalize) {
                    let previousHandoff = handoff
                    self = SessionState()
                    handoff = previousHandoff
                }
                connected = true
                if let next = event.address { address = BluetoothAddress.normalize(next) ?? next }
            } else {
                connected = false
                clearReadings()
            }
        case "audio":
            audio = event.state ?? "unknown"
            if audio == "disconnected" { holding = nil }
        case "headset":
            guard connected, let note = event.notification else { return }
            apply(note)
        case "handoff": handoff = event.progress
        default: break
        }
    }

    mutating func clearReadings() {
        battery = nil; firmware = nil; mac = nil; noise = nil; ambientVolume = nil
        effect = nil; gameMode = nil; ldac = nil; promptVolume = nil
        shutdownEnabled = nil; shutdownMinutes = nil; autoPowerOff = nil
        controlNormal = nil; controlReduction = nil; controlAmbient = nil
    }

    private mutating func apply(_ note: HeadsetNotification) {
        switch note.kind {
        case "battery": battery = note.percent.flatMap { (0...100).contains($0) ? $0 : nil }
        case "name": name = note.name
        case "firmware": firmware = note.version
        case "mac": mac = note.address.flatMap(BluetoothAddress.normalize)
        case "noise": noise = note.mode; ambientVolume = note.ambientVolume
        case "sound_effect": effect = note.effect
        case "game_mode": gameMode = note.on
        case "ldac": ldac = note.mode
        case "prompt_volume": promptVolume = note.volume
        case "shutdown_timer_enabled": shutdownEnabled = note.on
        case "shutdown_timer": shutdownMinutes = note.minutes; shutdownEnabled = true
        case "auto_power_off": autoPowerOff = note.on
        case "control_settings":
            controlNormal = note.normal; controlReduction = note.reduction; controlAmbient = note.ambient
        default: break
        }
    }
}

enum BluetoothAddress {
    static func normalize(_ input: String) -> String? {
        let trimmed = input.trimmingCharacters(in: .whitespacesAndNewlines)
        let compact = trimmed.replacingOccurrences(of: ":", with: "").replacingOccurrences(of: "-", with: "")
        guard compact.utf8.count == 12, compact.utf8.allSatisfy({ (48...57).contains($0) || (65...70).contains($0) || (97...102).contains($0) }) else { return nil }
        let chars = Array(compact.uppercased())
        return stride(from: 0, to: 12, by: 2).map { String(chars[$0...($0 + 1)]) }.joined(separator: ":")
    }
}

struct ActivityEntry: Identifiable {
    let id = UUID()
    let date = Date()
    let title: String
    let detail: String
    let isError: Bool
}

struct UserNotice: Identifiable {
    let id = UUID()
    let title: String
    let detail: String
    let isError: Bool
}

enum NativeJSON {
    static func decode<T: Decodable>(_ type: T.Type, _ text: String) throws -> T {
        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        return try decoder.decode(type, from: Data(text.utf8))
    }
}
