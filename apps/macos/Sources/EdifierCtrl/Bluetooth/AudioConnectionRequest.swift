import Foundation
import IOBluetooth
import IOKit

/// 请求对象单独持有回调状态. 超时后旧回调只能完成自己的请求, 不能覆盖下一次接管.
final class AudioConnectionRequest: NSObject {
    let id = UUID()
    let address: String
    private let device: IOBluetoothDevice
    private let condition = NSCondition()
    private var result: IOReturn?
    private var finished = false
    private let deadline = ProcessInfo.processInfo.systemUptime + 2.75

    // 以下登记表只在 main runloop 访问. 系统没有公开的 CREATE_CONNECTION 取消 API.
    private static var pending: [UUID: AudioConnectionRequest] = [:]
    private static var active: [String: UUID] = [:]

    init(device: IOBluetoothDevice, address: String) {
        self.device = device
        self.address = address
    }

    static func connect(address: String) throws {
        guard !Thread.isMainThread else {
            throw BluetoothFailure("音频连接必须从后台调用, main runloop 需要接收蓝牙回调")
        }
        if MacBluetooth.onMain({ CoreAudioBluetooth.state(address: address) == "connected" }) { return }
        let request = try MacBluetooth.onMain {
            guard active[address] == nil else { throw BluetoothFailure("该设备正在连接音频") }
            guard pending.count < 16 else {
                throw BluetoothFailure("系统尚有未完成的蓝牙连接请求, 请等待蓝牙回调后重试")
            }
            guard let device = IOBluetoothDevice(addressString: address) else {
                throw BluetoothFailure("找不到设备 \(address)")
            }
            let request = AudioConnectionRequest(device: device, address: address)
            active[address] = request.id
            pending[request.id] = request
            request.start()
            return request
        }
        defer { MacBluetooth.onMain { request.finish() } }
        try request.waitForConnection()
        try request.waitForAudio()
    }

    private func start() {
        NSLog("EdifierBt 请求系统连接音频 %@ request=%@", address, id.uuidString)
        // HCI Page Timeout 以 0.625ms 为单位, 3200 为 2 秒, 为端点发布保留时间.
        let status = device.openConnection(self, withPageTimeout: 3200, authenticationRequired: false)
        if status != kIOReturnSuccess {
            Self.pending.removeValue(forKey: id)
            // SDK 明确已经有 ACL 时可返回 Connection Exists, 仍须继续 CoreAudio 验证.
            complete(device.isConnected() ? kIOReturnSuccess : status)
        }
    }

    @objc func connectionComplete(_ sender: IOBluetoothDevice!, status: IOReturn) {
        guard canonicalBluetoothAddress(sender?.addressString ?? "") == address else { return }
        Self.pending.removeValue(forKey: id)
        guard Self.active[address] == id else { return }
        complete(status == kIOReturnSuccess || device.isConnected() ? kIOReturnSuccess : status)
    }

    private func complete(_ status: IOReturn) {
        condition.lock()
        if !finished, result == nil { result = status }
        condition.broadcast()
        condition.unlock()
    }

    private func waitForConnection() throws {
        condition.lock()
        defer { condition.unlock() }
        while result == nil && !finished {
            let remaining = deadline - ProcessInfo.processInfo.systemUptime
            if remaining <= 0 { break }
            _ = condition.wait(until: Date().addingTimeInterval(min(remaining, 0.05)))
        }
        guard !finished, let result else {
            throw BluetoothFailure("等待系统蓝牙连接回调超时, 未确认音频连接")
        }
        guard result == kIOReturnSuccess else {
            throw BluetoothFailure("系统蓝牙连接请求失败: \(result)")
        }
    }

    private func waitForAudio() throws {
        while ProcessInfo.processInfo.systemUptime < deadline {
            let state = MacBluetooth.onMain { CoreAudioBluetooth.state(address: address) }
            if state == "connected" {
                NSLog("EdifierBt 已确认蓝牙音频输出 %@ request=%@", address, id.uuidString)
                return
            }
            condition.lock()
            let remaining = deadline - ProcessInfo.processInfo.systemUptime
            if remaining > 0 && !finished {
                _ = condition.wait(until: Date().addingTimeInterval(min(remaining, 0.05)))
            }
            let cancelled = finished
            condition.unlock()
            if cancelled { break }
        }
        throw BluetoothFailure("已请求蓝牙连接, 但限时内未确认 CoreAudio 耳机输出. 系统可能未建立音频 profile")
    }

    private func finish() {
        condition.lock()
        finished = true
        condition.broadcast()
        condition.unlock()
        if Self.active[address] == id { Self.active.removeValue(forKey: address) }
        // 未回调的请求仍保留 delegate, 晚回调只移除同一 id 的登记, 不关闭新连接.
    }
}
