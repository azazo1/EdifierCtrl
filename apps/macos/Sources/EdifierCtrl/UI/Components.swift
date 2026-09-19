import SwiftUI

enum AppPage: String, CaseIterable, Identifiable {
    case headphones, devices, handoff, settings, activity
    var id: String { rawValue }
    var title: String {
        switch self {
        case .headphones: return "我的耳机"
        case .devices: return "设备连接"
        case .handoff: return "跨设备交接"
        case .settings: return "应用设置"
        case .activity: return "活动与诊断"
        }
    }
    var subtitle: String {
        switch self {
        case .headphones: return "让每一次聆听, 都恰到好处."
        case .devices: return "连接已配对的漫步者耳机, 开始使用."
        case .handoff: return "从手机到电脑, 让声音跟随你."
        case .settings: return "按你的习惯, 安静地在后台工作."
        case .activity: return "了解连接过程, 排查设备问题."
        }
    }
    var symbol: String {
        switch self {
        case .headphones: return "headphones"
        case .devices: return "antenna.radiowaves.left.and.right"
        case .handoff: return "arrow.triangle.swap"
        case .settings: return "slider.horizontal.3"
        case .activity: return "waveform.path.ecg"
        }
    }
}

enum Palette {
    static let accent = Color(red: 0.22, green: 0.43, blue: 0.91)
    static let mint = Color(red: 0.15, green: 0.65, blue: 0.48)
    static let canvas = Color(nsColor: .windowBackgroundColor)
    static let card = Color(nsColor: .controlBackgroundColor)
    static let border = Color.primary.opacity(0.075)
}

struct Surface<Content: View>: View {
    var padding: CGFloat = 22
    @ViewBuilder let content: Content
    var body: some View {
        content
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(padding)
            .background(Palette.card, in: RoundedRectangle(cornerRadius: 20, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: 20).stroke(Palette.border, lineWidth: 1).allowsHitTesting(false))
    }
}

struct SectionHeading: View {
    let title: String
    var detail: String? = nil
    var body: some View {
        VStack(alignment: .leading, spacing: 5) {
            Text(title).font(.system(size: 16, weight: .semibold))
            if let detail { Text(detail).font(.callout).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true) }
        }
    }
}

struct StatusPill: View {
    let text: String
    var color: Color = Palette.mint
    var body: some View {
        HStack(spacing: 6) {
            Circle().fill(color).frame(width: 6, height: 6)
            Text(text).font(.system(size: 11, weight: .medium))
        }
        .padding(.horizontal, 10).padding(.vertical, 6)
        .background(color.opacity(0.10), in: Capsule())
        .foregroundStyle(color)
        .accessibilityElement(children: .combine)
    }
}

struct EmptyState: View {
    let symbol: String
    let title: String
    let detail: String
    var body: some View {
        VStack(spacing: 14) {
            Image(systemName: symbol).font(.system(size: 38, weight: .light)).foregroundStyle(Palette.accent)
                .frame(width: 82, height: 82).background(Palette.accent.opacity(0.07), in: RoundedRectangle(cornerRadius: 26))
            Text(title).font(.title3.weight(.semibold))
            Text(detail).font(.callout).foregroundStyle(.secondary).multilineTextAlignment(.center).frame(maxWidth: 430)
        }
        .frame(maxWidth: .infinity).padding(.vertical, 30)
    }
}

struct Metric: View {
    let label: String
    let value: String
    let symbol: String
    var body: some View {
        HStack(spacing: 10) {
            Image(systemName: symbol).foregroundStyle(Palette.accent).frame(width: 22)
            VStack(alignment: .leading, spacing: 4) {
                Text(label).font(.caption).foregroundStyle(.secondary)
                Text(value).font(.system(size: 13, weight: .medium)).textSelection(.enabled)
            }
        }
    }
}

struct ChoiceTile: View {
    let title: String
    let subtitle: String
    let symbol: String
    let selected: Bool
    let action: () -> Void
    var body: some View {
        Button(action: action) {
            VStack(alignment: .leading, spacing: 12) {
                HStack {
                    Image(systemName: symbol).font(.system(size: 22, weight: .medium))
                    Spacer()
                    if selected { Image(systemName: "checkmark.circle.fill").font(.system(size: 16)) }
                }
                VStack(alignment: .leading, spacing: 4) {
                    Text(title).font(.system(size: 14, weight: .semibold))
                    Text(subtitle).font(.caption).foregroundStyle(.secondary)
                }
            }
            .foregroundStyle(selected ? Palette.accent : Color.primary)
            .padding(17).frame(maxWidth: .infinity, alignment: .leading)
        }
        .buttonStyle(ChoiceButtonStyle(selected: selected))
        .accessibilityLabel(title)
        .accessibilityValue(selected ? "已选中" : "未选中")
    }
}

/// 编辑时保留本地草稿, 只有明确应用才发送, 新回报不会覆盖正在编辑的值.
struct LevelControl: View {
    let title: String
    let detail: String
    let value: Int?
    let range: ClosedRange<Double>
    var unit: String = ""
    let enabled: Bool
    let apply: (Int) -> Void
    @State private var draft = 0.0
    @State private var dirty = false

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            HStack {
                SectionHeading(title: title, detail: detail)
                Spacer()
                Text(dirty ? "\(Int(draft))\(unit)" : value.map { "\($0)\(unit)" } ?? "未读取")
                    .font(.system(.body, design: .rounded).weight(.semibold)).monospacedDigit()
            }
            Slider(value: Binding(get: { draft }, set: { draft = $0; dirty = true }), in: range, step: 1)
                .disabled(!enabled).accessibilityLabel(title)
            HStack {
                Text("\(Int(range.lowerBound))").font(.caption).foregroundStyle(.secondary)
                Spacer()
                if dirty {
                    Button("撤销") { sync() }.buttonStyle(.plain).foregroundStyle(.secondary)
                    Button("应用") { apply(Int(draft)); dirty = false }.buttonStyle(.borderedProminent).disabled(!enabled)
                }
                Text("\(Int(range.upperBound))").font(.caption).foregroundStyle(.secondary)
            }
            .frame(height: 26)
        }
        .onAppear { sync() }
        .onChange(of: value) { _ in if !dirty { sync() } }
    }

    private func sync() { draft = min(range.upperBound, max(range.lowerBound, Double(value ?? Int(range.lowerBound)))) ; dirty = false }
}
