import Darwin
import Foundation
import IOBluetooth
import IOKit

/// 阻塞 C ABI 只能从后台调用. IOBluetooth 对象仅由 main runloop 操作.
enum MacBluetooth {
    static let rfcommUuid = "EDF00000-EDFE-DFED-FEDF-EDFEDFEDFEDF"
    static var serviceUUID: IOBluetoothSDPUUID? {
        guard var bytes = UUID(uuidString: rfcommUuid)?.uuid else { return nil }
        return withUnsafeBytes(of: &bytes) {
            IOBluetoothSDPUUID(bytes: $0.baseAddress!, length: $0.count)
        }
    }
    static var connection: RfcommConnection?
    private static let errorKey = "EdifierCtrl.BluetoothError"

    static func onMain<T>(_ body: () throws -> T) rethrows -> T {
        if Thread.isMainThread { return try body() }
        return try DispatchQueue.main.sync(execute: body)
    }

    static func setError(_ text: String) {
        Thread.current.threadDictionary[errorKey] = BluetoothErrorString(text)
        AppLog.error("蓝牙桥操作失败: \(text)", category: "bluetooth")
    }

    static func ok() {
        Thread.current.threadDictionary.removeObject(forKey: errorKey)
    }

    static var lastError: UnsafePointer<CChar>? {
        guard let value = Thread.current.threadDictionary[errorKey] as? BluetoothErrorString else { return nil }
        return UnsafePointer(value.pointer)
    }

    static func perform(_ body: () throws -> Void) -> Int32 {
        do {
            try body()
            ok()
            return 0
        } catch {
            setError(error.localizedDescription)
            return -1
        }
    }

    static func address(_ pointer: UnsafePointer<CChar>?) throws -> String {
        guard let pointer, let address = canonicalBluetoothAddress(String(cString: pointer)) else {
            throw BluetoothFailure("蓝牙地址无效")
        }
        return address
    }
}

struct BluetoothFailure: LocalizedError {
    let text: String
    init(_ text: String) { self.text = text }
    var errorDescription: String? { text }
}

private final class BluetoothErrorString {
    let pointer: UnsafeMutablePointer<CChar>?
    init(_ text: String) { pointer = strdup(text) }
    deinit { free(pointer) }
}

@_cdecl("edifier_macos_last_error")
public func edifier_macos_last_error() -> UnsafePointer<CChar>? {
    MacBluetooth.lastError
}

@_cdecl("edifier_macos_scan")
public func edifier_macos_scan() -> UnsafeMutablePointer<CChar>? {
    let rows: [[String: String]] = MacBluetooth.onMain {
        let devices = (IOBluetoothDevice.pairedDevices() as? [IOBluetoothDevice]) ?? []
        return devices.compactMap { device in
            guard let address = canonicalBluetoothAddress(device.addressString ?? "") else { return nil }
            return ["address": address, "name": device.nameOrAddress ?? "", "kind": "rfcomm",
                    "service_uuid": MacBluetooth.rfcommUuid.lowercased()]
        }
    }
    do {
        let data = try JSONSerialization.data(withJSONObject: rows)
        MacBluetooth.ok()
        return strdup(String(decoding: data, as: UTF8.self))
    } catch {
        MacBluetooth.setError("扫描序列化失败: \(error.localizedDescription)")
        return nil
    }
}

@_cdecl("edifier_macos_open")
public func edifier_macos_open(_ address: UnsafePointer<CChar>?) -> Int32 {
    MacBluetooth.perform {
        guard !Thread.isMainThread else {
            throw BluetoothFailure("RFCOMM open 必须从后台调用, main runloop 需要接收蓝牙回调")
        }
        let address = try MacBluetooth.address(address)
        let connection = try MacBluetooth.onMain {
            guard let device = IOBluetoothDevice(addressString: address) else {
                throw BluetoothFailure("无法创建蓝牙设备对象")
            }
            MacBluetooth.connection?.close()
            let connection = RfcommConnection(device: device, address: address)
            MacBluetooth.connection = connection
            connection.start()
            return connection
        }
        do {
            try connection.waitUntilOpen()
        } catch {
            MacBluetooth.onMain {
                connection.close()
                if MacBluetooth.connection === connection { MacBluetooth.connection = nil }
            }
            throw error
        }
    }
}

@_cdecl("edifier_macos_write")
public func edifier_macos_write(_ ptr: UnsafePointer<UInt8>?, _ len: Int32) -> Int32 {
    MacBluetooth.perform {
        guard let ptr, len > 0, len <= Int32(UInt16.max) else {
            throw BluetoothFailure("RFCOMM 写入长度无效")
        }
        var copy = Data(bytes: ptr, count: Int(len))
        try MacBluetooth.onMain {
            guard let connection = MacBluetooth.connection else { throw BluetoothFailure("尚未打开 RFCOMM") }
            try connection.write(&copy)
        }
    }
}

@_cdecl("edifier_macos_read")
public func edifier_macos_read(_ ptr: UnsafeMutablePointer<UInt8>?, _ cap: Int32) -> Int32 {
    guard !Thread.isMainThread, let ptr, cap > 0 else { return -1 }
    guard let connection = MacBluetooth.onMain({ MacBluetooth.connection }),
          let data = connection.take(max: Int(cap), timeoutMs: 200) else { return -1 }
    data.copyBytes(to: ptr, count: data.count)
    return Int32(data.count)
}

@_cdecl("edifier_macos_close")
public func edifier_macos_close() -> Int32 {
    MacBluetooth.perform {
        let status = MacBluetooth.onMain {
            let status = MacBluetooth.connection?.close() ?? kIOReturnSuccess
            MacBluetooth.connection = nil
            return status
        }
        guard status == kIOReturnSuccess else { throw BluetoothFailure("RFCOMM 关闭失败 \(status)") }
    }
}

@_cdecl("edifier_macos_audio_state")
public func edifier_macos_audio_state(_ pointer: UnsafePointer<CChar>?) -> UnsafeMutablePointer<CChar>? {
    do {
        let address = try MacBluetooth.address(pointer)
        let state = MacBluetooth.onMain { CoreAudioBluetooth.state(address: address) }
        MacBluetooth.ok()
        return strdup(state)
    } catch {
        MacBluetooth.setError(error.localizedDescription)
        return nil
    }
}

@_cdecl("edifier_macos_audio_connect")
public func edifier_macos_audio_connect(_ pointer: UnsafePointer<CChar>?) -> Int32 {
    MacBluetooth.perform {
        let address = try MacBluetooth.address(pointer)
        try AudioConnectionRequest.connect(address: address)
    }
}

@_cdecl("edifier_macos_audio_select_output")
public func edifier_macos_audio_select_output(_ pointer: UnsafePointer<CChar>?) -> Int32 {
    MacBluetooth.perform {
        let address = try MacBluetooth.address(pointer)
        try MacBluetooth.onMain { try CoreAudioBluetooth.selectOutput(address: address) }
    }
}

@_cdecl("edifier_macos_audio_disconnect")
public func edifier_macos_audio_disconnect(_ pointer: UnsafePointer<CChar>?) -> Int32 {
    MacBluetooth.perform {
        let address = try MacBluetooth.address(pointer)
        try MacBluetooth.onMain {
            guard let device = IOBluetoothDevice(addressString: address) else {
                throw BluetoothFailure("无法创建蓝牙设备对象")
            }
            // 公共 API 只能释放整条 ACL, 对应 RFCOMM 也必须立即结束.
            if MacBluetooth.connection?.address == address {
                MacBluetooth.connection?.close()
                MacBluetooth.connection = nil
            }
            let rc = device.closeConnection()
            guard rc == kIOReturnSuccess else { throw BluetoothFailure("蓝牙断开失败 \(rc)") }
        }
    }
}
