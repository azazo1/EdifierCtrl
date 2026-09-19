import Darwin
import Foundation

/// 运行时 dlopen `libedifier_ffi.dylib`, 对应 crates/edifier-ffi/include/edifier.h.
enum EdifierNative {
    private static let handle: UnsafeMutableRawPointer? = {
        let names = [
            "libedifier_ffi.dylib",
            "./libedifier_ffi.dylib",
            Bundle.main.bundlePath + "/libedifier_ffi.dylib",
        ]
        for name in names {
            if let h = dlopen(name, RTLD_NOW | RTLD_LOCAL) {
                return h
            }
        }
        return nil
    }()

    static var loaded: Bool { handle != nil }
    static var session: UnsafeMutableRawPointer?

    static func version() -> String {
        guard let fn: @convention(c) () -> UnsafePointer<CChar>? = symbol("edifier_version") else {
            return "未链接 edifier_ffi"
        }
        guard let p = fn() else { return "" }
        return String(cString: p)
    }

    static func encodeCommand(_ json: String) -> String {
        guard let fn: @convention(c) (UnsafePointer<CChar>?) -> UnsafeMutablePointer<CChar>? = symbol("edifier_command_encode") else {
            return "未链接 edifier_ffi"
        }
        return json.withCString { take(fn($0)) }
    }

    static func parseFrame(_ hex: String) -> String {
        guard let fn: @convention(c) (UnsafePointer<CChar>?) -> UnsafeMutablePointer<CChar>? = symbol("edifier_frame_parse") else {
            return "未链接 edifier_ffi"
        }
        return hex.withCString { take(fn($0)) }
    }

    static func ensureSession() {
        guard session == nil else { return }
        guard let fn: @convention(c) (UnsafePointer<CChar>?) -> UnsafeMutableRawPointer? = symbol("edifier_session_new") else {
            return
        }
        let id = Host.current().localizedName ?? "mac"
        session = id.withCString { fn($0) }
    }

    static func scan(_ kind: String) -> String {
        ensureSession()
        guard let s = session,
              let fn: @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<CChar>?) -> UnsafeMutablePointer<CChar>? = symbol("edifier_session_scan")
        else {
            return lastError()
        }
        return kind.withCString { take(fn(s, $0)) }
    }

    static func connect(_ address: String, kind: String) -> String {
        ensureSession()
        guard let s = session,
              let fn: @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<CChar>?, UnsafePointer<CChar>?) -> Int32 = symbol("edifier_session_connect")
        else {
            return lastError()
        }
        let rc = address.withCString { a in kind.withCString { k in fn(s, a, k) } }
        return rc == 0 ? "已连接 \(address)" : lastError()
    }

    static func disconnect() -> String {
        ensureSession()
        guard let s = session,
              let fn: @convention(c) (UnsafeMutableRawPointer?) -> Int32 = symbol("edifier_session_disconnect")
        else {
            return lastError()
        }
        return fn(s) == 0 ? "已断开控制通道" : lastError()
    }

    static func sendJson(_ json: String) -> String {
        ensureSession()
        guard let s = session,
              let fn: @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<CChar>?) -> Int32 = symbol("edifier_session_send_json")
        else {
            return lastError()
        }
        let rc = json.withCString { fn(s, $0) }
        return rc == 0 ? "已发送 \(json)" : lastError()
    }

    static func readout(_ profile: String = "basedevice") -> String {
        ensureSession()
        guard let s = session,
              let fn: @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<CChar>?) -> Int32 = symbol("edifier_session_readout")
        else {
            return lastError()
        }
        let rc = profile.withCString { fn(s, $0) }
        return rc == 0 ? "已发送读状态" : lastError()
    }

    static func pollEvent() -> String {
        ensureSession()
        guard let s = session,
              let fn: @convention(c) (UnsafeMutableRawPointer?) -> UnsafeMutablePointer<CChar>? = symbol("edifier_session_poll_event")
        else {
            return ""
        }
        return take(fn(s))
    }

    static func groupJoin(_ passphrase: String) -> String {
        ensureSession()
        guard let s = session,
              let fn: @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<CChar>?) -> Int32 = symbol("edifier_session_group_join")
        else {
            return lastError()
        }
        let rc = passphrase.withCString { fn(s, $0) }
        return rc == 0 ? "已加入组 holding=\(holding())" : lastError()
    }

    static func groupPeers() -> String {
        ensureSession()
        guard let s = session,
              let fn: @convention(c) (UnsafeMutableRawPointer?) -> UnsafeMutablePointer<CChar>? = symbol("edifier_session_group_peers")
        else {
            return lastError()
        }
        return take(fn(s))
    }

    static func groupClaim(_ mac: String) -> String {
        ensureSession()
        guard let s = session,
              let fn: @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<CChar>?) -> Int32 = symbol("edifier_session_group_claim")
        else {
            return lastError()
        }
        let rc = mac.withCString { fn(s, $0) }
        return rc == 0 ? "已请求接管 \(mac)" : lastError()
    }

    static func groupClaimPeer(_ peerId: String) -> String {
        ensureSession()
        guard let s = session,
              let fn: @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<CChar>?) -> Int32 = symbol("edifier_session_group_claim_peer")
        else {
            return lastError()
        }
        let rc = peerId.withCString { fn(s, $0) }
        return rc == 0 ? "已向成员请求接管" : lastError()
    }

    static func holding() -> String {
        ensureSession()
        guard let s = session,
              let fn: @convention(c) (UnsafeMutableRawPointer?) -> UnsafeMutablePointer<CChar>? = symbol("edifier_session_holding")
        else {
            return ""
        }
        return take(fn(s))
    }

    private static func lastError() -> String {
        guard let fn: @convention(c) () -> UnsafePointer<CChar>? = symbol("edifier_last_error") else {
            return "未链接 edifier_ffi"
        }
        guard let p = fn() else { return "未知 FFI 错误" }
        return String(cString: p)
    }

    private static func take(_ ptr: UnsafeMutablePointer<CChar>?) -> String {
        guard let ptr else { return lastError() }
        defer {
            if let free: @convention(c) (UnsafeMutablePointer<CChar>?) -> Void = symbol("edifier_string_free") {
                free(ptr)
            }
        }
        return String(cString: ptr)
    }

    private static func symbol<T>(_ name: String) -> T? {
        guard let handle, let raw = dlsym(handle, name) else { return nil }
        return unsafeBitCast(raw, to: T.self)
    }
}
