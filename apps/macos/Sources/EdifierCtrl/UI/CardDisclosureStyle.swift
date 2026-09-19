import SwiftUI

/// 折叠卡片的标题和留白共用一个按钮, 展开内容保持独立交互.
struct CardDisclosureStyle: DisclosureGroupStyle {
    func makeBody(configuration: Configuration) -> some View {
        Surface(padding: 0) {
            VStack(alignment: .leading, spacing: 0) {
                Button {
                    withAnimation(.easeInOut(duration: 0.16)) {
                        configuration.isExpanded.toggle()
                    }
                } label: {
                    HStack(spacing: 10) {
                        Image(systemName: "chevron.right")
                            .font(.system(size: 11, weight: .semibold))
                            .rotationEffect(.degrees(configuration.isExpanded ? 90 : 0))
                            .accessibilityHidden(true)
                        configuration.label
                        Spacer(minLength: 0)
                    }
                    .frame(maxWidth: .infinity, minHeight: 24, alignment: .leading)
                    .padding(22)
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .accessibilityValue(configuration.isExpanded ? "已展开" : "已折叠")
                if configuration.isExpanded {
                    configuration.content
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding([.horizontal, .bottom], 22)
                }
            }
        }
    }
}
