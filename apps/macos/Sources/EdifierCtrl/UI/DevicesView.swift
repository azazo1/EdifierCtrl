import AppKit
import SwiftUI

struct DevicesView: View {
    @ObservedObject var model: AppModel
    @State private var address = ""
    @State private var search = ""
    @State private var showAll = false

    private var filtered: [HeadphoneDevice] {
        model.devices.filter { device in
            (showAll || device.isEdifier || BluetoothAddress.normalize(device.address) == model.state.address)
                && (search.isEmpty || device.displayName.localizedCaseInsensitiveContains(search) || device.address.localizedCaseInsensitiveContains(search))
        }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 22) {
            Surface {
                HStack(alignment: .top, spacing: 18) {
                    Image(systemName: "antenna.radiowaves.left.and.right").font(.system(size: 26)).foregroundStyle(Palette.accent)
                        .frame(width: 54, height: 54).background(Palette.accent.opacity(0.08), in: RoundedRectangle(cornerRadius: 16))
                    VStack(alignment: .leading, spacing: 8) {
                        Text("从系统配对, 在这里连接").font(.headline)
                        Text("打开耳机并在 macOS 蓝牙设置中配对. 刷新列表后, 选择设备即可连接控制通道.")
                            .font(.callout).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
                        Button("打开蓝牙设置") { openBluetoothSettings() }.buttonStyle(.link)
                    }
                    Spacer(minLength: 0)
                }
            }
            HStack(spacing: 12) {
                TextField("搜索名称或蓝牙地址", text: $search).textFieldStyle(.roundedBorder).frame(maxWidth: 280)
                Toggle("显示所有设备", isOn: $showAll).toggleStyle(.checkbox).font(.callout)
                Spacer()
                Button {
                    model.scan()
                } label: { Label(model.scanning ? "正在刷新" : "刷新设备", systemImage: "arrow.clockwise") }
                    .buttonStyle(ActionStyle(prominent: true)).disabled(!model.ready || model.busy || model.scanning)
            }
            if filtered.isEmpty {
                Surface {
                    EmptyState(symbol: model.scanning ? "dot.radiowaves.left.and.right" : "headphones", title: model.scanning ? "正在读取配对设备" : "还没有找到耳机", detail: showAll ? "确认耳机已在系统中配对并打开, 然后刷新设备列表." : "请先刷新列表. 如果耳机使用了自定义名称, 可以开启显示所有设备.")
                }
            } else {
                ForEach(filtered) { device in
                    deviceRow(device)
                }
            }
            Surface {
                DisclosureGroup("使用蓝牙地址连接") {
                    VStack(alignment: .leading, spacing: 12) {
                        Text("适用于已经配对但未出现在列表中的经典蓝牙耳机. 当前 macOS 端使用 RFCOMM 控制通道.")
                            .font(.caption).foregroundStyle(.secondary)
                        HStack {
                            TextField("AA:BB:CC:DD:EE:FF", text: $address).textFieldStyle(.roundedBorder)
                                .font(.system(.body, design: .monospaced))
                                .onSubmit { if BluetoothAddress.normalize(address) != nil { model.connect(address: address) } }
                            Button("连接") { model.connect(address: address) }
                                .disabled(BluetoothAddress.normalize(address) == nil || !model.ready || model.busy)
                        }
                    }.padding(.top, 14)
                }
            }
        }
        .onAppear { if model.devices.isEmpty { model.scan() } }
    }

    private func deviceRow(_ device: HeadphoneDevice) -> some View {
        let connected = model.isConnected && BluetoothAddress.normalize(device.address) == model.state.address
        return Surface(padding: 18) {
            HStack(spacing: 16) {
                Image(systemName: "headphones").font(.system(size: 28, weight: .light)).foregroundStyle(Palette.accent)
                    .frame(width: 60, height: 60).background(Palette.accent.opacity(0.07), in: RoundedRectangle(cornerRadius: 16))
                VStack(alignment: .leading, spacing: 6) {
                    Text(device.displayName).font(.system(size: 15, weight: .semibold))
                    Text(device.address).font(.system(size: 11, design: .monospaced)).foregroundStyle(.secondary).textSelection(.enabled)
                }
                Spacer()
                if connected { StatusPill(text: "已连接") }
                Button(connected ? "断开控制" : "连接") {
                    if connected { model.disconnect() } else { model.connect(device) }
                }.buttonStyle(ActionStyle(prominent: !connected)).disabled(model.busy || !model.ready)
            }
        }
    }
}

func openBluetoothSettings() {
    guard let url = URL(string: "x-apple.systempreferences:com.apple.BluetoothSettings") else { return }
    NSWorkspace.shared.open(url)
}
