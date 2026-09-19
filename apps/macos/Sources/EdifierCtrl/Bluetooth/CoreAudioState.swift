import CoreAudio
import Foundation
import IOBluetooth

func canonicalBluetoothAddress(_ value: String) -> String? {
    let compact = value.filter { $0 != ":" && $0 != "-" && !$0.isWhitespace }
    guard compact.utf8.count == 12, compact.utf8.allSatisfy({
        (48...57).contains($0) || (65...70).contains($0) || (97...102).contains($0)
    }) else { return nil }
    let bytes = Array(compact.uppercased())
    return stride(from: 0, to: 12, by: 2).map { String(bytes[$0...($0 + 1)]) }.joined(separator: ":")
}

/// CoreAudio 的蓝牙输出端点证明音频 profile 可用, ACL 连接只能用来排除已断开的设备.
enum CoreAudioBluetooth {
    private enum Observation {
        case connected(AudioDeviceID)
        case disconnected
        case unknown
    }

    static func state(address: String) -> String {
        switch observe(address: address) {
        case .connected: return "connected"
        case .disconnected: return "disconnected"
        case .unknown: return "unknown"
        }
    }

    static func selectOutput(address: String) throws {
        guard case .connected(var id) = observe(address: address) else {
            throw BluetoothFailure("没有可用的耳机音频输出, 请先在系统蓝牙设置中连接")
        }
        let system = AudioObjectID(kAudioObjectSystemObject)
        if uintProperty(system, kAudioHardwarePropertyDefaultOutputDevice) == id { return }
        var property = AudioObjectPropertyAddress(mSelector: kAudioHardwarePropertyDefaultOutputDevice,
                                                  mScope: kAudioObjectPropertyScopeGlobal,
                                                  mElement: kAudioObjectPropertyElementMain)
        let status = AudioObjectSetPropertyData(system, &property, 0, nil,
                                               UInt32(MemoryLayout<AudioDeviceID>.size), &id)
        guard status == noErr,
              uintProperty(system, kAudioHardwarePropertyDefaultOutputDevice) == id else {
            throw BluetoothFailure("选择耳机为系统音频输出失败: \(status)")
        }
        NSLog("EdifierBt 已选择耳机音频输出 %@", address)
    }

    private static func observe(address: String) -> Observation {
        guard let device = IOBluetoothDevice(addressString: address) else { return .unknown }
        if !device.isConnected() { return .disconnected }
        var property = AudioObjectPropertyAddress(mSelector: kAudioHardwarePropertyDevices,
                                                  mScope: kAudioObjectPropertyScopeGlobal,
                                                  mElement: kAudioObjectPropertyElementMain)
        var size: UInt32 = 0
        guard AudioObjectGetPropertyDataSize(AudioObjectID(kAudioObjectSystemObject), &property, 0, nil, &size) == noErr,
              size >= MemoryLayout<AudioDeviceID>.size else { return .unknown }
        var devices = [AudioDeviceID](repeating: 0, count: Int(size) / MemoryLayout<AudioDeviceID>.size)
        let result = devices.withUnsafeMutableBytes {
            AudioObjectGetPropertyData(AudioObjectID(kAudioObjectSystemObject), &property, 0, nil, &size, $0.baseAddress!)
        }
        guard result == noErr else { return .unknown }
        var matched = false
        for id in devices {
            guard let transport = uintProperty(id, kAudioDevicePropertyTransportType),
                  transport == kAudioDeviceTransportTypeBluetooth || transport == kAudioDeviceTransportTypeBluetoothLE,
                  let uid = stringProperty(id, kAudioDevicePropertyDeviceUID),
                  uidMatches(uid, address: address) else { continue }
            matched = true
            guard let alive = uintProperty(id, kAudioDevicePropertyDeviceIsAlive) else { return .unknown }
            if alive == 0 { continue }
            var streams = AudioObjectPropertyAddress(mSelector: kAudioDevicePropertyStreams,
                                                     mScope: kAudioObjectPropertyScopeOutput,
                                                     mElement: kAudioObjectPropertyElementMain)
            var streamSize: UInt32 = 0
            guard AudioObjectGetPropertyDataSize(id, &streams, 0, nil, &streamSize) == noErr else { return .unknown }
            if streamSize >= MemoryLayout<AudioStreamID>.size { return .connected(id) }
        }
        // 不按名称猜地址, 不能匹配 UID 时保留未知, 避免把同名耳机混为一台.
        return matched ? .disconnected : .unknown
    }

    static func uidMatches(_ uid: String, address: String) -> Bool {
        let pattern = "(?i)(?<![0-9a-f])(?:[0-9a-f]{2}[:-]){5}[0-9a-f]{2}(?![0-9a-f])|^[0-9a-f]{12}(?=$|[:_-](?:output|input|a2dp|hfp)(?:$|[:_-]))"
        guard let expression = try? NSRegularExpression(pattern: pattern) else { return false }
        let text = uid as NSString
        return expression.matches(in: uid, range: NSRange(location: 0, length: text.length)).contains {
            canonicalBluetoothAddress(text.substring(with: $0.range)) == address
        }
    }

    private static func uintProperty(_ object: AudioObjectID, _ selector: AudioObjectPropertySelector) -> UInt32? {
        var property = AudioObjectPropertyAddress(mSelector: selector,
                                                  mScope: kAudioObjectPropertyScopeGlobal,
                                                  mElement: kAudioObjectPropertyElementMain)
        var value: UInt32 = 0
        var size = UInt32(MemoryLayout<UInt32>.size)
        return AudioObjectGetPropertyData(object, &property, 0, nil, &size, &value) == noErr ? value : nil
    }

    private static func stringProperty(_ object: AudioObjectID, _ selector: AudioObjectPropertySelector) -> String? {
        var property = AudioObjectPropertyAddress(mSelector: selector,
                                                  mScope: kAudioObjectPropertyScopeGlobal,
                                                  mElement: kAudioObjectPropertyElementMain)
        var value: Unmanaged<CFString>?
        var size = UInt32(MemoryLayout<Unmanaged<CFString>?>.size)
        guard AudioObjectGetPropertyData(object, &property, 0, nil, &size, &value) == noErr else { return nil }
        // DeviceUID 的所有权由 CoreAudio 转交调用者.
        return value?.takeRetainedValue() as String?
    }
}
