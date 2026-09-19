import Foundation
import IOBluetooth
import IOKit

/// 每条 RFCOMM 独立拥有缓冲和关闭状态, 重连不能复用上一条连接的数据.
final class RfcommConnection: NSObject, IOBluetoothRFCOMMChannelDelegate {
    let device: IOBluetoothDevice
    let address: String
    private var channel: IOBluetoothRFCOMMChannel?
    private let condition = NSCondition()
    private var buffer = Data()
    private var closed = false
    private var openResult: IOReturn?

    init(device: IOBluetoothDevice, address: String) {
        self.device = device
        self.address = address
    }

    func start() {
        MacBluetooth.pendingQueries[address] = self
        let status = device.performSDPQuery(self)
        if status != kIOReturnSuccess {
            MacBluetooth.pendingQueries.removeValue(forKey: address)
            complete(status)
        }
    }

    @objc func sdpQueryComplete(_ device: IOBluetoothDevice!, status: IOReturn) {
        MacBluetooth.pendingQueries.removeValue(forKey: address)
        condition.lock()
        let cancelled = closed
        condition.unlock()
        guard !cancelled else { return }
        guard status == kIOReturnSuccess,
              let uuid = MacBluetooth.serviceUUID,
              let record = device.getServiceRecord(for: uuid) else {
            complete(status == kIOReturnSuccess ? kIOReturnNotFound : status)
            return
        }
        var channelID: BluetoothRFCOMMChannelID = 0
        let lookup = record.getRFCOMMChannelID(&channelID)
        guard lookup == kIOReturnSuccess, channelID > 0 else {
            complete(lookup == kIOReturnSuccess ? kIOReturnNotFound : lookup)
            return
        }
        let opened = device.openRFCOMMChannelAsync(&channel, withChannelID: channelID, delegate: self)
        if opened != kIOReturnSuccess { complete(opened) }
    }

    func rfcommChannelOpenComplete(_ channel: IOBluetoothRFCOMMChannel!, status: IOReturn) {
        complete(status)
    }

    private func complete(_ status: IOReturn) {
        condition.lock()
        if !closed, openResult == nil { openResult = status }
        condition.broadcast()
        condition.unlock()
    }

    func waitUntilOpen() throws {
        condition.lock()
        defer { condition.unlock() }
        let deadline = Date().addingTimeInterval(12)
        while openResult == nil && !closed {
            if !condition.wait(until: deadline) { break }
        }
        guard !closed, openResult == kIOReturnSuccess else {
            throw BluetoothFailure("SDP/RFCOMM 连接失败或超时: \(openResult ?? kIOReturnTimeout)")
        }
    }

    func rfcommChannelData(_ channel: IOBluetoothRFCOMMChannel!, data pointer: UnsafeMutableRawPointer!, length: Int) {
        guard let pointer, length > 0 else { return }
        condition.lock()
        if closed {
            condition.unlock()
            return
        }
        guard length <= 65_536 - buffer.count else {
            condition.unlock()
            NSLog("EdifierBt RFCOMM 接收缓冲超限, 关闭通道")
            close()
            return
        }
        buffer.append(pointer.assumingMemoryBound(to: UInt8.self), count: length)
        condition.broadcast()
        condition.unlock()
    }

    func rfcommChannelClosed(_ channel: IOBluetoothRFCOMMChannel!) {
        close()
    }

    func take(max: Int, timeoutMs: Int) -> Data? {
        condition.lock()
        defer { condition.unlock() }
        let deadline = Date().addingTimeInterval(Double(timeoutMs) / 1000)
        while buffer.isEmpty && !closed {
            if !condition.wait(until: deadline) { break }
        }
        guard !closed else { return nil }
        let count = min(max, buffer.count)
        let result = Data(buffer.prefix(count))
        buffer.removeFirst(count)
        return result
    }

    func write(_ data: inout Data) throws {
        condition.lock()
        let writable = !closed && openResult == kIOReturnSuccess
        condition.unlock()
        guard writable, let channel else { throw BluetoothFailure("RFCOMM 已关闭") }
        let status = data.withUnsafeMutableBytes { raw -> IOReturn in
            guard let pointer = raw.baseAddress else { return kIOReturnBadArgument }
            return channel.writeSync(pointer, length: UInt16(raw.count))
        }
        if status != kIOReturnSuccess { throw BluetoothFailure("RFCOMM 写入失败 \(status)") }
    }

    @discardableResult
    func close() -> IOReturn {
        condition.lock()
        closed = true
        buffer.removeAll(keepingCapacity: false)
        condition.broadcast()
        condition.unlock()
        let old = channel
        channel = nil
        old?.setDelegate(nil)
        return old?.close() ?? kIOReturnSuccess
    }
}
