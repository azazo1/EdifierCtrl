import SwiftUI

/// 分栏背景贯穿标题栏, 只有侧边栏内容避开原生窗口按钮.
struct WorkspaceLayout<Sidebar: View, Content: View>: View {
    @ViewBuilder let sidebar: Sidebar
    @ViewBuilder let content: Content

    var body: some View {
        GeometryReader { geometry in
            HStack(spacing: 0) {
                sidebar
                    .padding(.top, geometry.safeAreaInsets.top)
                    .background(.ultraThinMaterial)
                Divider()
                content
            }
            // 右侧标题和分隔线延伸到顶边. 全屏时安全区由 AppKit 变为 0,
            // 不给原生全屏顶部栏施加普通窗口的按钮偏移.
            .ignoresSafeArea(.container, edges: .top)
        }
    }
}
