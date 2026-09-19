import SwiftUI

/// 文本操作按钮. 原生 Button 继续负责键盘激活和 disabled 行为.
struct ActionStyle: ButtonStyle {
    var prominent = false

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.system(size: 13, weight: .semibold))
            .padding(.horizontal, 16).padding(.vertical, 10)
            .foregroundStyle(prominent ? Color.white : Palette.accent)
            .modifier(ButtonFeedback(
                isPressed: configuration.isPressed,
                cornerRadius: 10,
                background: prominent ? Palette.accent : Palette.accent.opacity(0.08),
                border: prominent ? .clear : Palette.accent.opacity(0.12),
                prominent: prominent
            ))
    }
}

/// 尺寸就是完整点击区域, 不向相邻按钮或窗口拖动区域扩张.
struct IconButtonStyle: ButtonStyle {
    var width: CGFloat = 40
    var height: CGFloat = 38
    var cornerRadius: CGFloat = 10

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .frame(width: width, height: height)
            .modifier(ButtonFeedback(
                isPressed: configuration.isPressed,
                cornerRadius: cornerRadius,
                background: Palette.canvas,
                border: Palette.border
            ))
    }
}

/// 选项卡片的布局由 label 决定, 样式只负责状态反馈.
struct ChoiceButtonStyle: ButtonStyle {
    var selected = false
    var cornerRadius: CGFloat = 14

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .modifier(ButtonFeedback(
                isPressed: configuration.isPressed,
                cornerRadius: cornerRadius,
                background: selected ? Palette.accent.opacity(0.09) : Palette.canvas.opacity(0.6),
                border: selected ? Palette.accent.opacity(0.45) : Palette.border
            ))
    }
}

/// 导航, 版本入口和折叠标题共用. 不添加 padding, 保持调用方的布局与点击边界.
struct QuietButtonStyle: ButtonStyle {
    var selected = false
    var cornerRadius: CGFloat = 10

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .modifier(ButtonFeedback(
                isPressed: configuration.isPressed,
                cornerRadius: cornerRadius,
                background: selected ? Palette.accent.opacity(0.10) : .clear,
                border: .clear
            ))
    }
}

private struct ButtonFeedback: ViewModifier {
    @Environment(\.isEnabled) private var isEnabled
    @Environment(\.isFocused) private var isFocused
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var isHovered = false

    let isPressed: Bool
    let cornerRadius: CGFloat
    let background: Color
    let border: Color
    var prominent = false

    private var highlighted: Bool { isEnabled && (isHovered || isPressed) }
    private var focused: Bool { isEnabled && isFocused }
    private var shape: RoundedRectangle { RoundedRectangle(cornerRadius: cornerRadius, style: .continuous) }
    private var transition: Animation? { reduceMotion ? nil : .easeOut(duration: 0.12) }

    private var highlight: Color {
        guard isEnabled else { return .clear }
        if prominent {
            return isPressed ? Color.black.opacity(0.14) : isHovered ? Color.white.opacity(0.10) : .clear
        }
        return Palette.accent.opacity(isPressed ? 0.15 : isHovered ? 0.065 : 0)
    }

    func body(content: Content) -> some View {
        content
            .contentShape(shape)
            .background(background, in: shape)
            .overlay(shape.fill(highlight).allowsHitTesting(false))
            .overlay(shape.strokeBorder(highlighted ? Palette.accent.opacity(0.38) : border, lineWidth: 1).allowsHitTesting(false))
            // 焦点由系统 Button 提供, 不创建额外焦点节点, 不拦截 Tab 或空格事件.
            .overlay(shape.strokeBorder(focused ? Color.accentColor : .clear, lineWidth: 2).allowsHitTesting(false))
            .opacity(isEnabled ? 1 : 0.42)
            .onHover { isHovered = $0 }
            .animation(transition, value: isHovered)
            .animation(transition, value: isPressed)
            .animation(transition, value: focused)
    }
}
