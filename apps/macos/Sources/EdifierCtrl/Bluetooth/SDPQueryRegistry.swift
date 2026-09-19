import Foundation
import IOBluetooth
import IOKit

/// 活跃请求与系统仍可能调用的 target 分开管理. 所有登记操作在 main runloop 执行.
final class SDPQueryRegistry {
    static let shared = SDPQueryRegistry()
    private let maximumPending: Int
    private var active: [String: UUID] = [:]
    private var pending: [UUID: SDPQueryCallback] = [:]

    init(maximumPending: Int = 32) {
        self.maximumPending = maximumPending
    }

    func begin(address: String, completion: @escaping (IOBluetoothDevice?, IOReturn) -> Void) throws -> SDPQueryCallback {
        guard active[address] == nil else {
            throw BluetoothFailure("该设备正在查询控制服务, 请等待本次连接结束")
        }
        // SDK 没有取消 SDP 或保证 target retain 的契约, 不能定时释放尚未回调的对象.
        // 仅保留轻量 target, 并限制异常系统状态下的总数, 不无限保留连接与接收缓冲.
        guard pending.count < maximumPending else {
            AppLog.error("SDP 待回调请求达到上限 \(maximumPending), 需要系统返回回调或重启应用后重试.", category: "bluetooth")
            throw BluetoothFailure("系统积压的 SDP 回调过多, 请重启应用后重试控制连接")
        }
        let target = SDPQueryCallback(registry: self, address: address, completion: completion)
        active[address] = target.id
        pending[target.id] = target
        return target
    }

    func cancel(_ id: UUID) {
        guard let target = pending[id] else { return }
        releaseActive(target)
        target.completion = nil
        AppLog.debug("SDP 请求已取消, 保留晚回调目标, pending=\(pending.count).", category: "bluetooth")
    }

    /// performSDPQuery 返回失败意味着请求未提交, 此时无需继续保留 target.
    func reject(_ id: UUID, status: IOReturn) {
        finish(id, device: nil, status: status)
    }

    fileprivate func finish(_ id: UUID, device: IOBluetoothDevice?, status: IOReturn) {
        guard let target = pending.removeValue(forKey: id) else { return }
        let isActive = active[target.address] == id
        releaseActive(target)
        let completion = target.completion
        target.completion = nil
        guard isActive, let completion else {
            AppLog.debug("已丢弃取消请求的 SDP 晚回调, pending=\(pending.count).", category: "bluetooth")
            return
        }
        completion(device, status)
    }

    private func releaseActive(_ target: SDPQueryCallback) {
        if active[target.address] == target.id { active.removeValue(forKey: target.address) }
    }
}

/// 不持有连接对象. 超时后清空闭包, 保留至 SDK 的唯一完成回调到达.
final class SDPQueryCallback: NSObject {
    let id = UUID()
    fileprivate let address: String
    fileprivate var completion: ((IOBluetoothDevice?, IOReturn) -> Void)?
    private weak var registry: SDPQueryRegistry?

    fileprivate init(registry: SDPQueryRegistry, address: String, completion: @escaping (IOBluetoothDevice?, IOReturn) -> Void) {
        self.registry = registry
        self.address = address
        self.completion = completion
    }

    // IOBluetoothDeviceAsyncCallbacks 的正式 ObjC selector, 不依赖 Swift 名称推导.
    @objc(sdpQueryComplete:status:)
    func sdpQueryComplete(_ device: IOBluetoothDevice?, status: IOReturn) {
        MacBluetooth.onMain { registry?.finish(id, device: device, status: status) }
    }
}
