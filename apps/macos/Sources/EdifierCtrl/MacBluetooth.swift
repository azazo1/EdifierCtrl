import Darwin
import Foundation
import IOBluetooth
import IOKit

/// IOBluetooth 桥, 符号名给 Rust `edifier-bt-macos` dlsym.
enum MacBluetooth {
    static let rfcommUuid = "EDF00000-EDFE-DFED-FEDF-EDFEDFEDFEDF"
    static let sink = RfcommSink()
    static var channel: IOBluetoothRFCOMMChannel?
    static var lastErrorC: UnsafeMutablePointer<CChar>?

    static func setError(_ text: String) {
        if let old = lastErrorC {
            free(old)
        }
        lastErrorC = strdup(text)
        NSLog("EdifierBt %@", text)
    }

    static func ok() {
        if let old = lastErrorC {
            free(old)
        }
        lastErrorC = nil
    }
}

final class RfcommSink: NSObject, IOBluetoothRFCOMMChannelDelegate {
    private let lock = NSLock()
    private var buf = Data()
    private let cond = NSCondition()

    func rfcommChannelData(_ rfcommChannel: IOBluetoothRFCOMMChannel!, data dataPointer: UnsafeMutableRawPointer!, length dataLength: Int) {
        lock.lock()
        buf.append(Data(bytes: dataPointer, count: dataLength))
        lock.unlock()
        cond.broadcast()
    }

    func rfcommChannelClosed(_ rfcommChannel: IOBluetoothRFCOMMChannel!) {
        cond.broadcast()
    }

    func take(max: Int, timeoutMs: Int) -> Data {
        cond.lock()
        defer { cond.unlock() }
        let deadline = Date().addingTimeInterval(Double(timeoutMs) / 1000.0)
        while true {
            lock.lock()
            if !buf.isEmpty {
                let n = min(max, buf.count)
                let out = buf.prefix(n)
                buf.removeFirst(n)
                lock.unlock()
                return Data(out)
            }
            lock.unlock()
            if !cond.wait(until: deadline) {
                return Data()
            }
        }
    }
}

@_cdecl("edifier_macos_last_error")
public func edifier_macos_last_error() -> UnsafePointer<CChar>? {
    UnsafePointer(MacBluetooth.lastErrorC)
}

@_cdecl("edifier_macos_scan")
public func edifier_macos_scan() -> UnsafeMutablePointer<CChar>? {
    let devices = (IOBluetoothDevice.pairedDevices() as? [IOBluetoothDevice]) ?? []
    var rows: [[String: String]] = []
    for d in devices {
        rows.append([
            "address": d.addressString ?? "",
            "name": d.nameOrAddress ?? "",
            "kind": "rfcomm",
            "service_uuid": "edf00000-edfe-dfed-fedf-edfedfedfedf",
        ])
    }
    guard let data = try? JSONSerialization.data(withJSONObject: rows),
          let text = String(data: data, encoding: .utf8)
    else {
        MacBluetooth.setError("扫描序列化失败")
        return nil
    }
    MacBluetooth.ok()
    return strdup(text)
}

@_cdecl("edifier_macos_open")
public func edifier_macos_open(_ address: UnsafePointer<CChar>?) -> Int32 {
    guard let address else {
        MacBluetooth.setError("地址为空")
        return -1
    }
    let addr = String(cString: address)
    guard let device = IOBluetoothDevice(addressString: addr) else {
        MacBluetooth.setError("找不到设备 \(addr)")
        return -1
    }
    _ = edifier_macos_close()
    if !device.isConnected() {
        let rc = device.openConnection()
        if rc != kIOReturnSuccess {
            MacBluetooth.setError("openConnection 失败 \(rc)")
            return -1
        }
    }
    var channelId: BluetoothRFCOMMChannelID = 1
    if let uuid = IOBluetoothSDPUUID(uuidString: MacBluetooth.rfcommUuid),
       let record = device.getServiceRecord(for: uuid)
    {
        var found: BluetoothRFCOMMChannelID = 0
        if record.getRFCOMMChannelID(&found) == kIOReturnSuccess, found != 0 {
            channelId = found
        }
    }
    var channel: IOBluetoothRFCOMMChannel?
    let rc = device.openRFCOMMChannelSync(&channel, withChannelID: channelId, delegate: MacBluetooth.sink)
    guard rc == kIOReturnSuccess, let channel else {
        MacBluetooth.setError("RFCOMM 通道 \(channelId) 失败 \(rc)")
        return -1
    }
    MacBluetooth.channel = channel
    MacBluetooth.ok()
    return 0
}

@_cdecl("edifier_macos_write")
public func edifier_macos_write(_ ptr: UnsafePointer<UInt8>?, _ len: Int32) -> Int32 {
    guard let channel = MacBluetooth.channel, let ptr, len > 0 else {
        MacBluetooth.setError("尚未打开 RFCOMM")
        return -1
    }
    var copy = Data(bytes: ptr, count: Int(len))
    let rc: IOReturn = copy.withUnsafeMutableBytes { raw in
        guard let base = raw.baseAddress else { return kIOReturnBadArgument }
        return channel.writeSync(base, length: UInt16(len))
    }
    if rc != kIOReturnSuccess {
        MacBluetooth.setError("RFCOMM 写入失败 \(rc)")
        return -1
    }
    return 0
}

@_cdecl("edifier_macos_read")
public func edifier_macos_read(_ ptr: UnsafeMutablePointer<UInt8>?, _ cap: Int32) -> Int32 {
    guard let ptr, cap > 0 else { return 0 }
    let data = MacBluetooth.sink.take(max: Int(cap), timeoutMs: 200)
    data.copyBytes(to: ptr, count: data.count)
    return Int32(data.count)
}

@_cdecl("edifier_macos_close")
public func edifier_macos_close() -> Int32 {
    MacBluetooth.channel?.close()
    MacBluetooth.channel = nil
    return 0
}

@_cdecl("edifier_macos_audio_state")
public func edifier_macos_audio_state(_ address: UnsafePointer<CChar>?) -> UnsafeMutablePointer<CChar>? {
    guard let address, let device = IOBluetoothDevice(addressString: String(cString: address)) else {
        return strdup("disconnected")
    }
    return strdup(device.isConnected() ? "connected" : "disconnected")
}

@_cdecl("edifier_macos_audio_connect")
public func edifier_macos_audio_connect(_ address: UnsafePointer<CChar>?) -> Int32 {
    guard let address, let device = IOBluetoothDevice(addressString: String(cString: address)) else {
        MacBluetooth.setError("找不到设备")
        return -1
    }
    let rc = device.openConnection()
    if rc != kIOReturnSuccess && !device.isConnected() {
        MacBluetooth.setError("音频连接失败 \(rc)")
        return -1
    }
    MacBluetooth.ok()
    return 0
}

@_cdecl("edifier_macos_audio_disconnect")
public func edifier_macos_audio_disconnect(_ address: UnsafePointer<CChar>?) -> Int32 {
    guard let address, let device = IOBluetoothDevice(addressString: String(cString: address)) else {
        MacBluetooth.setError("找不到设备")
        return -1
    }
    let rc = device.closeConnection()
    if rc != kIOReturnSuccess {
        MacBluetooth.setError("音频断开失败 \(rc)")
        return -1
    }
    MacBluetooth.ok()
    return 0
}
