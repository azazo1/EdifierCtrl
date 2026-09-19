import Darwin
import Foundation

/// POSIX 处理器只忽略默认终止, DispatchSource 将信号送回主线程命令入口.
@MainActor
final class TerminationSignals {
    private var sources: [DispatchSourceSignal] = []

    func start(requestQuit: @escaping (String) -> Void) {
        guard sources.isEmpty else { return }
        for value in [SIGINT, SIGTERM] {
            signal(value, SIG_IGN)
            let source = DispatchSource.makeSignalSource(signal: value, queue: .main)
            source.setEventHandler {
                let name = value == SIGINT ? "SIGINT" : "SIGTERM"
                Task { @MainActor in requestQuit(name) }
            }
            source.resume()
            sources.append(source)
        }
    }

    func stop() {
        sources.forEach { $0.cancel() }
        sources.removeAll()
        // 退出收尾期间继续忽略默认信号动作, 避免第二次 Ctrl+C 打断资源清理.
    }
}
