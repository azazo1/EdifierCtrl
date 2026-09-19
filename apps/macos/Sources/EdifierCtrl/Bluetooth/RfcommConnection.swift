import Foundation
import IOBluetooth
import IOKit

/// 每条 RFCOMM 独立拥有缓冲和关闭状态, 重连不能复用上一条连接的数据.
final class RfcommConnection: NSObject, IOBluetoothRFCOMMChannelDelegate {
    let device: IOBluetoothDevice
    let address: String
    private var channel: IOBluetoothRFCOMMChannel?
    private var queryID: UUID?
    private var usingCachedRecord = false
    private var cachedOpenTimeout: DispatchWorkItem?
    private let condition = NSCondition()
    private var buffer = Data()
    private var closed = false
    private var openResult: IOReturn?
    private var failure: String?
    private var stage = "SDP 服务查询"
    private let startedAt = ProcessInfo.processInfo.systemUptime

    init(device: IOBluetoothDevice, address: String) {
        self.device = device
        self.address = address
    }

    func start() {
        AppLog.info("开始连接耳机 RFCOMM 控制通道.", category: "bluetooth")
        if let uuid = MacBluetooth.serviceUUID, let record = device.getServiceRecord(for: uuid) {
            var channelID: BluetoothRFCOMMChannelID = 0
            if record.getRFCOMMChannelID(&channelID) == kIOReturnSuccess, channelID > 0 {
                usingCachedRecord = true
                AppLog.info("系统已有控制服务记录, 尝试打开缓存的 RFCOMM 通道.", category: "bluetooth")
                openChannel(channelID)
                return
            }
        }
        startQuery()
    }

    private func startQuery() {
        usingCachedRecord = false
        condition.lock()
        let cancelled = closed
        if !cancelled { stage = "SDP 服务查询" }
        condition.unlock()
        guard !cancelled else { return }
        AppLog.info("开始查询耳机 RFCOMM 控制服务, 耗时 \(elapsed).", category: "bluetooth")
        guard let uuid = MacBluetooth.serviceUUID else {
            complete(kIOReturnBadArgument, failure: "控制服务 UUID 无效")
            return
        }
        do {
            let target = try SDPQueryRegistry.shared.begin(address: address) { [weak self] device, status in
                self?.sdpCompleted(device, status: status)
            }
            queryID = target.id
            let status = device.performSDPQuery(target, uuids: [uuid])
            if status != kIOReturnSuccess {
                SDPQueryRegistry.shared.reject(target.id, status: status)
            }
        } catch {
            complete(kIOReturnNoResources, failure: error.localizedDescription)
        }
    }

    private func sdpCompleted(_ sender: IOBluetoothDevice?, status: IOReturn) {
        queryID = nil
        condition.lock()
        let cancelled = closed
        condition.unlock()
        guard !cancelled else { return }
        AppLog.info("SDP 查询已返回, 耗时 \(elapsed), status=\(status).", category: "bluetooth")
        guard status == kIOReturnSuccess else {
            complete(status, failure: "SDP 控制服务查询失败")
            return
        }
        guard let sender, canonicalBluetoothAddress(sender.addressString ?? "") == address else {
            complete(kIOReturnBadArgument, failure: "SDP 回调与请求设备不匹配")
            return
        }
        guard let uuid = MacBluetooth.serviceUUID, let record = sender.getServiceRecord(for: uuid) else {
            complete(kIOReturnNotFound, failure: "设备没有返回漫步者 RFCOMM 控制服务")
            return
        }
        var channelID: BluetoothRFCOMMChannelID = 0
        let lookup = record.getRFCOMMChannelID(&channelID)
        guard lookup == kIOReturnSuccess, channelID > 0 else {
            complete(lookup == kIOReturnSuccess ? kIOReturnNotFound : lookup, failure: "控制服务没有可用的 RFCOMM 通道")
            return
        }
        openChannel(channelID)
    }

    private func openChannel(_ channelID: BluetoothRFCOMMChannelID) {
        condition.lock()
        let cancelled = closed
        if !cancelled { stage = "RFCOMM 通道打开" }
        condition.unlock()
        guard !cancelled else { return }
        AppLog.info("已找到控制服务, 开始异步打开 RFCOMM 通道, 耗时 \(elapsed).", category: "bluetooth")
        let opened = device.openRFCOMMChannelAsync(&channel, withChannelID: channelID, delegate: self)
        if opened != kIOReturnSuccess { openFailed(opened) }
        else if usingCachedRecord {
            // 缓存通道可能已失效. 为新查询保留时间, 避免每次重试都卡在同一缓存记录.
            let timeout = DispatchWorkItem { [weak self] in
                guard let self, self.usingCachedRecord else { return }
                self.openFailed(kIOReturnTimeout)
            }
            cachedOpenTimeout = timeout
            DispatchQueue.main.asyncAfter(deadline: .now() + 3, execute: timeout)
        }
    }

    private func openFailed(_ status: IOReturn) {
        cachedOpenTimeout?.cancel()
        cachedOpenTimeout = nil
        if usingCachedRecord {
            AppLog.info("缓存的 RFCOMM 通道未能打开, 改为重新查询服务, status=\(status), 耗时 \(elapsed).", category: "bluetooth")
            let old = channel
            channel = nil
            old?.setDelegate(nil)
            old?.close()
            startQuery()
        } else {
            complete(status, failure: "RFCOMM 控制通道连接失败")
        }
    }

    func rfcommChannelOpenComplete(_ sender: IOBluetoothRFCOMMChannel!, status: IOReturn) {
        condition.lock()
        let cancelled = closed
        condition.unlock()
        guard !cancelled else {
            sender?.setDelegate(nil)
            sender?.close()
            return
        }
        guard let sender, sender === channel else {
            AppLog.debug("忽略已替换 RFCOMM 通道的打开回调.", category: "bluetooth")
            return
        }
        if status == kIOReturnSuccess {
            cachedOpenTimeout?.cancel()
            cachedOpenTimeout = nil
            usingCachedRecord = false
            complete(status)
        } else { openFailed(status) }
    }

    private var elapsed: String {
        String(format: "%.2f 秒", ProcessInfo.processInfo.systemUptime - startedAt)
    }

    private func complete(_ status: IOReturn, failure: String? = nil) {
        condition.lock()
        let accepted = !closed && openResult == nil
        if accepted {
            openResult = status
            self.failure = failure
        }
        condition.broadcast()
        condition.unlock()
        guard accepted else { return }
        if status == kIOReturnSuccess {
            AppLog.info("RFCOMM 控制通道已打开, 总耗时 \(elapsed).", category: "bluetooth")
        } else {
            AppLog.error("\(failure ?? "控制连接失败"), status=\(status), 耗时 \(elapsed).", category: "bluetooth")
        }
    }

    func waitUntilOpen() throws {
        condition.lock()
        let deadline = startedAt + 12
        while openResult == nil && !closed {
            let remaining = deadline - ProcessInfo.processInfo.systemUptime
            if remaining <= 0 { break }
            _ = condition.wait(until: Date().addingTimeInterval(min(remaining, 0.2)))
        }
        if !closed, openResult == kIOReturnSuccess {
            condition.unlock()
            return
        }
        let message = failure ?? "\(stage)超时或已取消"
        let status = openResult ?? kIOReturnTimeout
        // 在后台等待结束时立即标记失效, 不给排队中的旧回调打开通道的机会.
        closed = true
        condition.broadcast()
        condition.unlock()
        throw BluetoothFailure("\(message): \(status), 耗时 \(elapsed). 可重试控制连接")
    }

    func rfcommChannelData(_ channel: IOBluetoothRFCOMMChannel!, data pointer: UnsafeMutableRawPointer!, length: Int) {
        guard channel === self.channel, let pointer, length > 0 else { return }
        condition.lock()
        if closed {
            condition.unlock()
            return
        }
        guard length <= 65_536 - buffer.count else {
            condition.unlock()
            AppLog.error("RFCOMM 接收缓冲超限, 关闭控制通道.", category: "bluetooth")
            close()
            return
        }
        buffer.append(pointer.assumingMemoryBound(to: UInt8.self), count: length)
        condition.broadcast()
        condition.unlock()
    }

    func rfcommChannelClosed(_ channel: IOBluetoothRFCOMMChannel!) {
        guard channel === self.channel else { return }
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
        cachedOpenTimeout?.cancel()
        cachedOpenTimeout = nil
        usingCachedRecord = false
        if let queryID {
            SDPQueryRegistry.shared.cancel(queryID)
            self.queryID = nil
        }
        condition.lock()
        let wasClosed = closed
        closed = true
        buffer.removeAll(keepingCapacity: false)
        condition.broadcast()
        condition.unlock()
        let old = channel
        channel = nil
        old?.setDelegate(nil)
        let status = old?.close() ?? kIOReturnSuccess
        if !wasClosed { AppLog.debug("RFCOMM 控制通道已关闭, status=\(status).", category: "bluetooth") }
        return status
    }
}
