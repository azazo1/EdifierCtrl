import Darwin
import Foundation

struct NativeFailure: LocalizedError {
    let message: String
    var errorDescription: String? { message }
}

struct NativeSnapshot {
    let events: [NativeEvent]
    let peers: [GroupPeer]?
    let holding: String?
}

/// C ABI 的句柄和字符串仅在此串行队列中访问, 主线程始终可以处理蓝牙回调.
final class EdifierNative: @unchecked Sendable {
    private let queue = DispatchQueue(label: "dev.edifierctrl.native", qos: .userInitiated)
    private let localID: String
    private var library: UnsafeMutableRawPointer?
    private var session: UnsafeMutableRawPointer?
    private var stopped = false

    init(localID: String) { self.localID = localID }

    func prepare() async throws -> (String, [HeadphoneProfile]) {
        try await perform {
            try self.loadLibrary()
            let installLog: NativeLogging.Install = try self.symbol("edifier_log_install")
            let logLevel: NativeLogging.SetLevel = try self.symbol("edifier_log_set_level")
            try NativeLogging.install(installLog, setLevel: logLevel)
            let version: @convention(c) () -> UnsafePointer<CChar>? = try self.symbol("edifier_version")
            let profiles: @convention(c) () -> UnsafeMutablePointer<CChar>? = try self.symbol("edifier_profiles_json")
            let create: @convention(c) (UnsafePointer<CChar>?) -> UnsafeMutableRawPointer? = try self.symbol("edifier_session_new")
            if self.session == nil { self.session = self.localID.withCString { create($0) } }
            guard self.session != nil else { throw self.lastError() }
            return (version().map { String(cString: $0) } ?? "未知", try NativeJSON.decode([HeadphoneProfile].self, self.take(profiles())))
        }
    }

    func scan() async throws -> [HeadphoneDevice] {
        try await perform {
            let fn: @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<CChar>?) -> UnsafeMutablePointer<CChar>? = try self.symbol("edifier_session_scan")
            return try "rfcomm".withCString { try NativeJSON.decode([HeadphoneDevice].self, self.take(fn(self.session, $0))) }
        }
    }

    func connect(_ address: String) async throws {
        try await perform {
            let fn: @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<CChar>?, UnsafePointer<CChar>?) -> Int32 = try self.symbol("edifier_session_connect")
            let code = address.withCString { address in "rfcomm".withCString { fn(self.session, address, $0) } }
            try self.check(code)
        }
    }

    func disconnect() async throws { try await call("edifier_session_disconnect") }
    func readout(_ profile: String) async throws { try await call("edifier_session_readout", text: profile) }
    func send(_ json: String) async throws { try await call("edifier_session_send_json", text: json) }
    func join(_ secret: String) async throws { try await call("edifier_session_group_join", text: secret) }
    func leave() async throws { try await call("edifier_session_group_leave") }
    func claimPeer(_ id: String) async throws { try await call("edifier_session_group_claim_peer", text: id) }
    func claimAddress(_ address: String) async throws { try await call("edifier_session_group_claim", text: address) }

    func poll(groupJoined: Bool, includePeers: Bool) async throws -> NativeSnapshot {
        try await perform {
            let poll: @convention(c) (UnsafeMutableRawPointer?) -> UnsafeMutablePointer<CChar>? = try self.symbol("edifier_session_poll_event")
            var events: [NativeEvent] = []
            for _ in 0..<64 {
                let event = try NativeJSON.decode(NativeEvent.self, self.take(poll(self.session)))
                if event.kind == "empty" { break }
                events.append(event)
            }
            var peers: [GroupPeer]?
            var holding: String?
            if groupJoined && includePeers {
                let members: @convention(c) (UnsafeMutableRawPointer?) -> UnsafeMutablePointer<CChar>? = try self.symbol("edifier_session_group_peers")
                peers = try NativeJSON.decode([GroupPeer].self, self.take(members(self.session)))
            }
            let held: @convention(c) (UnsafeMutableRawPointer?) -> UnsafeMutablePointer<CChar>? = try self.symbol("edifier_session_holding")
            let raw = try self.take(held(self.session))
            holding = raw.isEmpty ? nil : BluetoothAddress.normalize(raw)
            return NativeSnapshot(events: events, peers: peers, holding: holding)
        }
    }

    func inspect(_ input: String, frame: Bool) async throws -> String {
        try await perform {
            let fn: @convention(c) (UnsafePointer<CChar>?) -> UnsafeMutablePointer<CChar>? = try self.symbol(frame ? "edifier_frame_parse" : "edifier_command_encode")
            let raw = try input.withCString { try self.take(fn($0)) }
            guard let object = try? JSONSerialization.jsonObject(with: Data(raw.utf8)),
                  let pretty = try? JSONSerialization.data(withJSONObject: object, options: [.prettyPrinted, .sortedKeys]) else { return raw }
            return String(decoding: pretty, as: UTF8.self)
        }
    }

    func shutdown() async {
        await withCheckedContinuation { continuation in
            queue.async {
                self.stopped = true
                if let session = self.session {
                    if let free: @convention(c) (UnsafeMutableRawPointer?) -> Void = try? self.symbol("edifier_session_free") { free(session) }
                    self.session = nil
                }
                // Rust 可能保留运行时线程局部析构器, 进程退出前不卸载其动态库.
                continuation.resume()
            }
        }
    }

    private func call(_ name: String) async throws {
        try await perform {
            let fn: @convention(c) (UnsafeMutableRawPointer?) -> Int32 = try self.symbol(name)
            try self.check(fn(self.session))
        }
    }

    private func call(_ name: String, text: String) async throws {
        try await perform {
            let fn: @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<CChar>?) -> Int32 = try self.symbol(name)
            try self.check(text.withCString { fn(self.session, $0) })
        }
    }

    private func perform<T>(_ operation: @escaping () throws -> T) async throws -> T {
        try await withCheckedThrowingContinuation { continuation in
            queue.async {
                guard !self.stopped else {
                    continuation.resume(throwing: NativeFailure(message: "服务正在退出"))
                    return
                }
                continuation.resume(with: Result { try operation() })
            }
        }
    }

    private func loadLibrary() throws {
        guard library == nil else { return }
        let bundle = Bundle.main
        let executable = bundle.executableURL?.deletingLastPathComponent()
        let candidates = [
            bundle.privateFrameworksURL?.appendingPathComponent("libedifier_ffi.dylib"),
            executable?.appendingPathComponent("libedifier_ffi.dylib"),
        ].compactMap { $0 }
        for url in candidates {
            if let handle = dlopen(url.path, RTLD_NOW | RTLD_LOCAL) { library = handle; return }
        }
        throw NativeFailure(message: "无法加载耳机服务. 请使用完整的 EdifierCtrl.app, 或重新执行 just macos build.")
    }

    private func symbol<T>(_ name: String) throws -> T {
        guard let library, let raw = dlsym(library, name) else {
            throw NativeFailure(message: "耳机服务缺少接口 \(name), 请重新构建完整应用.")
        }
        return unsafeBitCast(raw, to: T.self)
    }

    private func check(_ code: Int32) throws { if code != 0 { throw lastError() } }

    private func lastError() -> NativeFailure {
        let fn: (@convention(c) () -> UnsafePointer<CChar>?)? = try? symbol("edifier_last_error")
        let message = fn?().map { String(cString: $0) } ?? "耳机服务未返回详细错误"
        return NativeFailure(message: message.isEmpty ? "耳机操作未完成" : message)
    }

    private func take(_ pointer: UnsafeMutablePointer<CChar>?) throws -> String {
        guard let pointer else { throw lastError() }
        let free: @convention(c) (UnsafeMutablePointer<CChar>?) -> Void = try symbol("edifier_string_free")
        defer { free(pointer) }
        return String(cString: pointer)
    }
}
