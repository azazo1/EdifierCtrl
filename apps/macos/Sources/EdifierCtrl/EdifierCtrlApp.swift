import Combine
import SwiftUI

@main
struct EdifierCtrlApp: App {
    var body: some Scene {
        WindowGroup {
            ContentView()
        }
    }
}

struct ContentView: View {
    var body: some View {
        TabView {
            DeviceView().tabItem { Text("设备") }
            ControlView().tabItem { Text("控制") }
            GroupView().tabItem { Text("组") }
            DebugView().tabItem { Text("调试") }
        }
        .frame(minWidth: 520, minHeight: 400)
    }
}

struct ScanDevice: Identifiable, Decodable {
    let address: String
    var name: String?
    var kind: String?
    var id: String { address }
}

struct DeviceView: View {
    @State private var log = EdifierNative.version()
    @State private var address = ""
    @State private var kind = "rfcomm"
    @State private var devices: [ScanDevice] = []
    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("设备").font(.title)
            Text(log)
            Button("扫描 RFCOMM") { scan("rfcomm") }
            Button("扫描 BLE") { scan("ble") }
            ForEach(devices) { row in
                Button("\(row.address)  \(row.name ?? "")") {
                    kind = row.kind ?? kind
                    log = EdifierNative.connect(row.address, kind: kind)
                }
            }
            TextField("耳机地址", text: $address)
            Button("连接 RFCOMM") { log = EdifierNative.connect(address, kind: "rfcomm") }
            Button("断开控制") { log = EdifierNative.disconnect() }
        }
        .padding(24)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
    }

    private func scan(_ k: String) {
        kind = k
        let json = EdifierNative.scan(k)
        log = json
        devices = decodeDevices(json)
    }
}

private func decodeDevices(_ json: String) -> [ScanDevice] {
    guard let data = json.data(using: .utf8),
          let rows = try? JSONDecoder().decode([ScanDevice].self, from: data)
    else {
        return []
    }
    return rows
}

struct ControlView: View {
    @State private var log = "先连接耳机."
    @State private var confirmCd = false
    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("控制").font(.title)
            Button("降噪关") { log = sendOrEncode(#"{"op":"set_noise_mode","mode":"normal"}"#) }
            Button("降噪") { log = sendOrEncode(#"{"op":"set_noise_mode","mode":"reduction"}"#) }
            Button("查电量") { log = sendOrEncode(#"{"op":"query_battery"}"#) }
            Button("读取状态") { log = EdifierNative.readout() }
            Button("拉事件") { log = EdifierNative.pollEvent() + "\n" + log }
            Button("断开主机 (CD)") { confirmCd = true }
            Text(log)
        }
        .padding(24)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .onReceive(Timer.publish(every: 0.25, on: .main, in: .common).autoconnect()) { _ in
            let ev = EdifierNative.pollEvent()
            if !ev.isEmpty && !ev.contains("empty") {
                log = ev + "\n" + log
            }
        }
        .confirmationDialog("确认", isPresented: $confirmCd) {
            Button("发送", role: .destructive) {
                log = sendOrEncode(#"{"op":"disconnect_host"}"#)
            }
            Button("取消", role: .cancel) {}
        } message: {
            Text("发 CD 会断开当前主机. 交接回退可以自动发, 这里是手动.")
        }
    }
}

private func sendOrEncode(_ json: String) -> String {
    let sent = EdifierNative.sendJson(json)
    if sent.hasPrefix("已发送") {
        return sent
    }
    return sent + "\n" + EdifierNative.encodeCommand(json)
}

struct GroupPeer: Identifiable, Decodable {
    let id: String
    var hostname: String?
    var holding: String?
}

struct GroupView: View {
    @State private var pass = ""
    @State private var mac = ""
    @State private var log = "加入后点某个成员接管其耳机."
    @State private var peers: [GroupPeer] = []
    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("组").font(.title)
            TextField("组口令", text: $pass)
            Button("加入") { log = EdifierNative.groupJoin(pass) }
            Button("刷新成员") { peers = decodePeers(EdifierNative.groupPeers()) }
            ForEach(peers) { peer in
                Button("\(peer.hostname ?? peer.id)  holding=\(peer.holding ?? "-")") {
                    log = EdifierNative.groupClaimPeer(peer.id)
                }
            }
            TextField("耳机 MAC", text: $mac)
            Button("接管音频") { log = EdifierNative.groupClaim(mac) }
            Text(log)
        }
        .padding(24)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
    }
}

private func decodePeers(_ json: String) -> [GroupPeer] {
    guard let data = json.data(using: .utf8),
          let rows = try? JSONDecoder().decode([GroupPeer].self, from: data)
    else {
        return []
    }
    return rows
}

struct DebugView: View {
    @State private var payload = #"{"op":"query_battery"}"#
    @State private var log = ""
    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("调试").font(.title)
            TextField("JSON 或 hex", text: $payload)
            Button("封装命令") { log = EdifierNative.encodeCommand(payload) }
            Button("解析帧") { log = EdifierNative.parseFrame(payload) }
            Text(log)
        }
        .padding(24)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
    }
}
