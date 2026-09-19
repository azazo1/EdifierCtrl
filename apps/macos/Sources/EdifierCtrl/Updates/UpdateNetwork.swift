import CryptoKit
import Darwin
import Foundation

final class UpdateRedirectGuard: NSObject, URLSessionTaskDelegate, @unchecked Sendable {
    func urlSession(_ session: URLSession, task: URLSessionTask, willPerformHTTPRedirection response: HTTPURLResponse, newRequest request: URLRequest, completionHandler: @escaping (URLRequest?) -> Void) {
        completionHandler(request.url?.scheme == "https" ? request : nil)
    }
}

// 每次操作创建新 session, 让 URLSession 使用当前系统代理设置.
enum UpdateHTTP {
    static func session() -> URLSession {
        URLSession(configuration: configuration(), delegate: UpdateRedirectGuard(), delegateQueue: nil)
    }

    static func configuration() -> URLSessionConfiguration {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.timeoutIntervalForRequest = 30
        configuration.timeoutIntervalForResource = 60 * 60
        configuration.requestCachePolicy = .reloadIgnoringLocalCacheData
        configuration.httpAdditionalHeaders = ["User-Agent": "EdifierCtrl-Updater", "Accept-Encoding": "identity"]
        return configuration
    }

    static func data(from url: URL, session: URLSession, limit: Int) async throws -> Data {
        var request = URLRequest(url: url)
        request.setValue("application/vnd.github+json", forHTTPHeaderField: "Accept")
        let (bytes, response) = try await session.bytes(for: request)
        try validate(response)
        var data = Data()
        for try await byte in bytes {
            try Task.checkCancellation()
            guard data.count < limit else { throw UpdateFailure("更新服务器返回的内容过大.") }
            data.append(byte)
        }
        return data
    }

    static func validate(_ response: URLResponse) throws {
        guard let response = response as? HTTPURLResponse else { throw UpdateFailure("更新服务器没有返回 HTTP 响应.") }
        if response.statusCode == 403 || response.statusCode == 429 {
            throw UpdateFailure("GitHub 请求受到限制. 请稍后重试.")
        }
        guard response.statusCode == 200 else {
            throw UpdateFailure("更新请求失败, HTTP \(response.statusCode). 请检查网络或系统代理.")
        }
        guard response.url?.scheme == "https" else { throw UpdateFailure("更新请求被重定向到了不安全的地址.") }
    }
}

enum UpdateFiles {
    static func directory(dataDirectory: URL) throws -> URL {
        let directory = dataDirectory.appendingPathComponent("update", isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let values = try directory.resourceValues(forKeys: [.isDirectoryKey, .isSymbolicLinkKey])
        guard values.isDirectory == true, values.isSymbolicLink != true else {
            throw UpdateFailure("更新目录不是有效的本地目录.")
        }
        return directory
    }

    static func regularFile(_ url: URL) throws -> Bool {
        var metadata = stat()
        if lstat(url.path, &metadata) != 0 {
            if errno == ENOENT { return false }
            throw POSIXError(POSIXErrorCode(rawValue: errno) ?? .EIO)
        }
        guard metadata.st_mode & S_IFMT == S_IFREG else { throw UpdateFailure("更新缓存路径必须是普通文件, 不能是链接或目录.") }
        return true
    }

    static func hash(_ url: URL, isCancelled: () -> Bool = { false }) throws -> String {
        let descriptor = open(url.path, O_RDONLY | O_NOFOLLOW | O_NONBLOCK)
        guard descriptor >= 0 else { throw UpdateFailure("无法打开更新包进行校验.") }
        let handle = FileHandle(fileDescriptor: descriptor, closeOnDealloc: true)
        defer { try? handle.close() }
        var metadata = stat()
        guard fstat(descriptor, &metadata) == 0, metadata.st_mode & S_IFMT == S_IFREG else {
            throw UpdateFailure("更新包不是普通文件.")
        }
        var digest = SHA256()
        while let data = try handle.read(upToCount: 1024 * 1024), !data.isEmpty {
            try Task.checkCancellation()
            if isCancelled() { throw CancellationError() }
            digest.update(data: data)
        }
        return digest.finalize().map { String(format: "%02x", $0) }.joined()
    }

    static func hashAsync(_ url: URL) async throws -> String {
        let worker = Task.detached(priority: .utility) { try hash(url) }
        return try await withTaskCancellationHandler {
            try await worker.value
        } onCancel: {
            worker.cancel()
        }
    }

    static func verifiedArchive(_ candidate: UpdateCandidate, directory: URL) async throws -> URL? {
        let archive = directory.appendingPathComponent(candidate.archive.name)
        guard try regularFile(archive) else { return nil }
        if try await hashAsync(archive) == candidate.digest { return archive }
        try FileManager.default.removeItem(at: archive)
        return nil
    }
}

private final class UpdateCancellation: @unchecked Sendable {
    private let lock = NSLock()
    private var value = false
    var isCancelled: Bool {
        lock.lock()
        defer { lock.unlock() }
        return value
    }
    func cancel() {
        lock.lock()
        value = true
        lock.unlock()
    }
}

// 文件写入和 delegate 回调在同一串行队列执行. 取消完成后才恢复 continuation.
final class UpdateDownload: NSObject, URLSessionDataDelegate, @unchecked Sendable {
    private let candidate: UpdateCandidate
    private let directory: URL
    private let progress: @Sendable (Int64, Int64) -> Void
    private let queue: OperationQueue = {
        let queue = OperationQueue()
        queue.maxConcurrentOperationCount = 1
        queue.qualityOfService = .utility
        return queue
    }()
    private var session: URLSession?
    private var task: URLSessionDataTask?
    private var handle: FileHandle?
    private var continuation: CheckedContinuation<URL, Error>?
    private var received: Int64 = 0
    private var failure: Error?
    private let cancellation = UpdateCancellation()
    private var lastProgress: Int64 = 0
    private var partial: URL { directory.appendingPathComponent(candidate.archive.name + ".part") }

    init(candidate: UpdateCandidate, directory: URL, progress: @escaping @Sendable (Int64, Int64) -> Void) {
        self.candidate = candidate
        self.directory = directory
        self.progress = progress
    }

    func run() async throws -> URL {
        try await withTaskCancellationHandler {
            try await withCheckedThrowingContinuation { continuation in
                queue.addOperation {
                    self.continuation = continuation
                    do {
                        guard !self.cancellation.isCancelled else { throw CancellationError() }
                        try self.prepare()
                    } catch { self.finish(.failure(error)) }
                }
            }
        } onCancel: { self.cancel() }
    }

    func cancel() {
        cancellation.cancel()
        queue.addOperation {
            self.task?.cancel()
        }
    }

    private func prepare() throws {
        let descriptor = open(partial.path, O_RDWR | O_CREAT | O_NOFOLLOW | O_NONBLOCK, S_IRUSR | S_IWUSR)
        guard descriptor >= 0 else { throw UpdateFailure("无法写入更新缓存.") }
        let handle = FileHandle(fileDescriptor: descriptor, closeOnDealloc: true)
        self.handle = handle
        var metadata = stat()
        guard fstat(descriptor, &metadata) == 0, metadata.st_mode & S_IFMT == S_IFREG else {
            throw UpdateFailure("更新缓存不是普通文件.")
        }
        received = Int64(metadata.st_size)
        if received >= candidate.archive.size {
            try handle.truncate(atOffset: 0)
            received = 0
        }
        try handle.seek(toOffset: UInt64(received))
        var request = URLRequest(url: candidate.archive.browserDownloadURL)
        if received > 0 { request.setValue("bytes=\(received)-", forHTTPHeaderField: "Range") }
        let session = URLSession(configuration: UpdateHTTP.configuration(), delegate: self, delegateQueue: queue)
        self.session = session
        task = session.dataTask(with: request)
        progress(received, candidate.archive.size)
        task?.resume()
    }

    func urlSession(_ session: URLSession, dataTask: URLSessionDataTask, didReceive response: URLResponse, completionHandler: @escaping (URLSession.ResponseDisposition) -> Void) {
        do {
            guard let http = response as? HTTPURLResponse, http.url?.scheme == "https" else {
                throw UpdateFailure("更新下载返回了无效或不安全的响应.")
            }
            if http.statusCode == 206 {
                let expected = "bytes \(received)-\(candidate.archive.size - 1)/\(candidate.archive.size)"
                guard http.value(forHTTPHeaderField: "Content-Range") == expected else {
                    throw UpdateFailure("服务器返回的续传范围无效, 请重新下载.")
                }
            } else {
                try UpdateHTTP.validate(response)
                try handle?.truncate(atOffset: 0)
                try handle?.seek(toOffset: 0)
                received = 0
            }
            if response.expectedContentLength >= 0,
               response.expectedContentLength != candidate.archive.size - received {
                throw UpdateFailure("下载大小与 Release 资产大小不一致.")
            }
            completionHandler(.allow)
        } catch {
            failure = error
            completionHandler(.cancel)
        }
    }

    func urlSession(_ session: URLSession, task: URLSessionTask, willPerformHTTPRedirection response: HTTPURLResponse, newRequest request: URLRequest, completionHandler: @escaping (URLRequest?) -> Void) {
        guard request.url?.scheme == "https" else {
            failure = UpdateFailure("拒绝不安全的下载重定向.")
            completionHandler(nil)
            return
        }
        completionHandler(request)
    }

    func urlSession(_ session: URLSession, dataTask: URLSessionDataTask, didReceive data: Data) {
        guard failure == nil, !cancellation.isCancelled else { return }
        do {
            guard Int64(data.count) <= candidate.archive.size - received else { throw UpdateFailure("下载超出预期大小.") }
            try handle?.write(contentsOf: data)
            received += Int64(data.count)
            progress(received, candidate.archive.size)
            if received - lastProgress >= 16 * 1024 * 1024 {
                lastProgress = received
                let current = received
                let total = candidate.archive.size
                Task { @MainActor in AppLog.debug("更新下载进度: \(current)/\(total) bytes") }
            }
        } catch {
            failure = error
            dataTask.cancel()
        }
    }

    func urlSession(_ session: URLSession, task: URLSessionTask, didCompleteWithError error: Error?) {
        do {
            try handle?.synchronize()
            try handle?.close()
            handle = nil
            if cancellation.isCancelled { throw CancellationError() }
            if let failure { throw failure }
            if let error { throw error }
            guard received == candidate.archive.size else { throw UpdateFailure("安装包下载不完整, 可稍后续传.") }
            let digest = try UpdateFiles.hash(partial) { self.cancellation.isCancelled }
            guard digest == candidate.digest else {
                try FileManager.default.removeItem(at: partial)
                throw UpdateFailure("SHA256 校验失败. 已删除不完整或被修改的下载文件.")
            }
            let final = directory.appendingPathComponent(candidate.archive.name)
            if try UpdateFiles.regularFile(final) { try FileManager.default.removeItem(at: final) }
            try FileManager.default.moveItem(at: partial, to: final)
            finish(.success(final))
        } catch { finish(.failure(error)) }
    }

    private func finish(_ result: Result<URL, Error>) {
        try? handle?.close()
        handle = nil
        session?.invalidateAndCancel()
        session = nil
        task = nil
        continuation?.resume(with: result)
        continuation = nil
    }
}
