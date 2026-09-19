package dev.edifierctrl.app.session

import android.content.Context
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.os.SystemClock
import android.util.Log
import dev.edifierctrl.app.BluetoothBridge
import dev.edifierctrl.app.EdifierNative
import dev.edifierctrl.app.infrastructure.AppPreferences
import org.json.JSONObject
import java.util.concurrent.Callable
import java.util.concurrent.ExecutionException
import java.util.concurrent.FutureTask
import java.util.concurrent.ScheduledFuture
import java.util.concurrent.ScheduledThreadPoolExecutor
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicBoolean

/** 应用进程拥有会话队列, Activity 销毁或旋转不会取消操作或另开轮询. */
object AppActions {
    private const val TAG = "EdifierActions"
    private val main = Handler(Looper.getMainLooper())
    private val worker = ScheduledThreadPoolExecutor(1) { task -> Thread(task, "edifier-session") }.apply {
        removeOnCancelPolicy = true
    }
    private val native = NativeSession()
    private val stopping = AtomicBoolean(false)
    private var pump: ScheduledFuture<*>? = null
    private var pollTick = 0
    private var pollFailures = 0

    // 生命周期决策和交接状态只在主线程更新, JNI 句柄只在 worker 内更新.
    private var initialized = false
    private var preferencesReady = false
    private var permissionSeen = false
    private var userStopped = false
    private var restartAfterStop = false
    private var startPending = false
    private var claimTarget: String? = null
    private var handoffStarted: Long? = null
    private var handoffDelayReported = false
    private var claimDone = false

    fun initialize(context: Context) {
        val application = context.applicationContext
        postMain {
            if (initialized) return@postMain
            initialized = true
            AppPreferences.initialize(application)
            worker.execute {
                val available = EdifierNative.ensureLoaded()
                onMain { SessionState.nativeAvailable = available }
            }
            AppPreferences.whenReady {
                preferencesReady = true
                AppPreferences.saveError?.let { SessionState.notify("读取设置失败", it, true) }
                if (SessionState.permissionsGranted && !userStopped) requestStart()
            }
        }
    }

    fun updateBluetoothPermission(granted: Boolean) = postMain {
        val changed = !permissionSeen || SessionState.permissionsGranted != granted
        permissionSeen = true
        SessionState.permissionsGranted = granted
        if (!granted) {
            if (changed && (SessionState.ready || SessionState.operation != null)) stopSession(false)
        } else if (changed && !userStopped) {
            if (stopping.get()) restartAfterStop = true else requestStart()
        }
    }

    fun start() = postMain {
        userStopped = false
        if (stopping.get()) {
            restartAfterStop = true
        } else {
            requestStart()
        }
    }

    fun stop() = postMain { stopSession(true) }

    private fun requestStart() {
        if (userStopped) return
        if (SessionState.ready) {
            startPending = false
            return
        }
        startPending = true
        if (!initialized || !preferencesReady || SessionState.operation != null || stopping.get()) return
        if (!SessionState.permissionsGranted) {
            SessionState.notify("需要蓝牙权限", "授权后才能启动耳机连接和交接服务", true)
            return
        }
        startPending = false
        SessionState.operation = "正在启动耳机服务"
        SessionState.record("正在启动耳机服务")
        val localId = "${Build.MODEL}-${AppPreferences.installationId}"
        worker.execute {
            var success = false
            try {
                ensureUsable()
                BluetoothBridge.start()
                ensureUsable()
                val (version, profiles) = native.prepare(localId)
                onMain {
                    SessionState.nativeAvailable = EdifierNative.loaded
                    if (!stopping.get() && SessionState.permissionsGranted) {
                        SessionState.initialize(
                            version, profiles, AppPreferences.selectedProfile,
                            AppPreferences.lastDeviceAddress, AppPreferences.lastDeviceName,
                        )
                        if (AppPreferences.selectedProfile != SessionState.selectedProfile.id) {
                            AppPreferences.selectProfile(SessionState.selectedProfile.id)
                        }
                        success = true
                    }
                }
                if (success && !stopping.get()) {
                    try {
                        attachExistingAudio()
                    } catch (error: Throwable) {
                        if (error is SessionStopping) throw error
                        report("启动时连接已有耳机失败", error)
                    }
                    ensureUsable()
                    pollTick = 0
                    pollFailures = 0
                    pump?.cancel(false)
                    pump = worker.scheduleWithFixedDelay(::poll, 0, 250, TimeUnit.MILLISECONDS)
                }
            } catch (error: Throwable) {
                success = false
                if (error !is SessionStopping) report("耳机服务启动失败", error)
                closeNative()
                onMain {
                    SessionState.ready = false
                    SessionState.nativeAvailable = EdifierNative.loaded
                }
            } finally {
                onMain {
                    if (!stopping.get()) SessionState.operation = null
                    if (success && !stopping.get() && SessionState.permissionsGranted && !userStopped) autoJoin()
                    if (startPending && !stopping.get() && SessionState.permissionsGranted && !userStopped) requestStart()
                }
            }
        }
    }

    private fun attachExistingAudio() {
        if (stopping.get()) return
        val connected = NativeSession.parseDevices(BluetoothBridge.connectedEdifier())
        if (connected.isEmpty()) return
        onMain { SessionState.updateDevices(connected) }
        val lastAddress = onMain { normalizeAddress(AppPreferences.lastDeviceAddress) }
        val selected = connected.firstOrNull { sameAddress(it.address, lastAddress) } ?: connected.first()
        Log.i(TAG, "启动时接管已连接音频: ${selected.address}")
        connectOnWorker(selected.address, selected.name, selected.kind)
    }

    private fun stopSession(explicit: Boolean) {
        if (explicit) {
            userStopped = true
            restartAfterStop = false
            startPending = false
        }
        if (!stopping.compareAndSet(false, true)) return
        SessionState.operation = if (explicit) "正在停止耳机服务" else "权限已撤销, 正在关闭耳机服务"
        // 取消未来轮询, 已进入的 JNI 不打断. 关闭排在同一 worker 队列末尾.
        worker.execute {
            pump?.cancel(false)
            pump = null
            closeNative()
            onMain {
                resetHandoff()
                SessionState.stop()
                stopping.set(false)
                val restart = restartAfterStop && !userStopped && SessionState.permissionsGranted
                restartAfterStop = false
                if (restart) requestStart()
            }
        }
    }

    fun scan(kind: String = "rfcomm") = run("正在查找已配对的耳机") {
        validateKind(kind)
        val devices = native.scan(kind)
        onMain {
            SessionState.updateDevices(devices)
            SessionState.record("设备列表已更新", "发现 ${devices.size} 台已配对设备")
        }
    }

    fun connect(address: String, name: String = "", kind: String = "rfcomm") = run("正在连接耳机") {
        validateKind(kind)
        val normalized = requireAddress(address)
        onMain { resetHandoff() }
        connectOnWorker(normalized, name, kind)
    }

    private fun connectOnWorker(address: String, name: String, kind: String = "rfcomm") {
        ensureUsable()
        native.connect(address, kind)
        val profile = onMain {
            SessionState.setConnected(true, address, name)
            val device = SessionState.devices.firstOrNull { sameAddress(it.address, address) }
            val resolvedName = name.ifBlank { device?.name.orEmpty() }
            val compactName = resolvedName.replace(" ", "")
            val remembered = if (sameAddress(AppPreferences.lastDeviceAddress, address)) {
                SessionState.profiles.firstOrNull { it.id == AppPreferences.selectedProfile }
            } else null
            val detected = remembered ?: SessionState.profiles.firstOrNull {
                it.serviceUuid != null && it.serviceUuid.equals(device?.serviceUuid, ignoreCase = true)
            } ?: SessionState.profiles.firstOrNull {
                it.id == "w820nbdoublegold" && (
                    compactName.contains("双金") || compactName.contains("doublegold", ignoreCase = true)
                )
            } ?: SessionState.profiles.sortedByDescending { it.id.length }.firstOrNull {
                it.id != "basedevice" && compactName.contains(it.id, ignoreCase = true)
            }
            if (detected != null && detected.id != SessionState.selectedProfile.id) SessionState.selectProfile(detected)
            val selected = SessionState.selectedProfile.id
            AppPreferences.rememberDevice(address, resolvedName, selected)
            SessionState.notify("控制通道已连接", "正在读取耳机状态. 系统音频会单独显示.")
            selected
        }
        ensureUsable()
        native.readout(profile)
    }

    fun disconnect() = run("正在断开控制通道") {
        native.disconnect()
        onMain {
            resetHandoff()
            SessionState.setConnected(false)
            SessionState.notify("控制通道已断开", "系统音频保持当前连接. 转移声音请使用跨设备交接.")
        }
    }

    fun readout() = run("正在读取耳机状态", requireConnection = true) {
        val profile = onMain { SessionState.selectedProfile.id }
        native.readout(profile)
        onMain { SessionState.record("已请求读取状态", "读数以耳机回报为准") }
    }

    fun selectProfile(id: String) = run("正在切换机型") {
        val connected = onMain {
            val selected = SessionState.profiles.firstOrNull { it.id == id } ?: error("机型档案不存在")
            SessionState.selectProfile(selected)
            AppPreferences.selectProfile(selected.id)
            SessionState.connected
        }
        if (connected) native.readout(id)
    }

    fun send(op: String, values: JSONObject = JSONObject(), label: String = "发送指令", query: String? = null) {
        // 入队前复制调用方参数, 避免页面草稿后续变化影响已确认指令.
        val snapshot = try {
            values.toString()
        } catch (error: Throwable) {
            postMain { SessionState.notify("指令参数无效", error.message.orEmpty(), true) }
            return
        }
        run(label, requireConnection = true) {
            val command = CommandValidation.build(op, JSONObject(snapshot))
            val followup = query?.takeIf { it.isNotBlank() }?.let { CommandValidation.build(it, JSONObject()) }
            val profile = onMain { SessionState.selectedProfile }
            CommandValidation.validate(command, profile)
            if (followup != null) {
                require(followup.getString("op").startsWith("query_")) { "后续读取必须使用查询指令" }
                CommandValidation.validate(followup, profile)
            }
            native.send(command.toString())
            onMain { SessionState.notify("指令已发送", "$label. 状态以耳机回报为准.") }
            if (followup != null) {
                ensureUsable()
                native.send(followup.toString())
            }
        }
    }

    fun join(group: String, remember: Boolean) = run("正在加入交接组") {
        require(group.isNotBlank()) { "请输入组名, 并在各设备上使用完全相同的内容" }
        require('\u0000' !in group) { "组名不能包含空字符" }
        onMain { check(!SessionState.groupJoined) { "请先退出当前交接组" } }
        native.join(group)
        onMain {
            SessionState.setGroup(true, group)
            AppPreferences.rememberJoinedGroup(group, remember)
            SessionState.notify("已加入交接组", "同一网络中使用相同组名的设备会自动出现在这里")
        }
        consume(native.poll(true, onMain { SessionState.address }))
    }

    fun leave() = run("正在退出交接组", allowDuringHandoff = true) {
        native.leave()
        onMain {
            resetHandoff()
            SessionState.setGroup(false)
            SessionState.notify("已退出交接组", "本机耳机控制仍可继续使用")
        }
    }

    fun claimPeer(id: String) = run("正在请求接管") {
        val peer = onMain {
            requireGroup()
            SessionState.peers.firstOrNull { it.id == id } ?: error("该组成员已经离线")
        }
        check(peer.canAudio && peer.holding != null) { "该成员没有可交接的系统音频连接" }
        beginClaim(requireAddress(peer.holding), peer.displayName) { native.claimPeer(peer.id) }
    }

    fun claim(address: String) = run("正在请求接管") {
        val normalized = requireAddress(address)
        onMain { requireGroup() }
        beginClaim(normalized, normalized) { native.claim(normalized) }
    }

    private fun beginClaim(address: String, label: String, action: () -> Unit) {
        onMain {
            check(!sameAddress(SessionState.holding, address)) { "本机已经持有这副耳机的系统音频" }
            claimTarget = address
            claimDone = false
            handoffStarted = SystemClock.elapsedRealtime()
            SessionState.setHandoff("requesting")
        }
        try {
            action()
            onMain { SessionState.record("请求接管耳机", label) }
        } catch (error: Throwable) {
            onMain {
                resetHandoff()
                SessionState.setHandoff("failed", error.message)
            }
            throw error
        }
    }

    fun diagnostic(input: String, parse: Boolean) = run(
        if (parse) "正在解析耳机数据" else "正在编码指令",
        requireBluetooth = false,
        requireReady = false,
    ) {
        val result = native.diagnostic(input, parse)
        onMain { SessionState.setDiagnostic(result) }
    }

    fun clearNotice() = postMain { SessionState.clearNotice() }
    fun clearActivities() = postMain { SessionState.clearActivities() }

    private fun autoJoin() {
        if (AppPreferences.rememberGroup && AppPreferences.autoJoinGroup && AppPreferences.groupName.isNotBlank()
            && !SessionState.groupJoined && !SessionState.busy
        ) join(AppPreferences.groupName, true)
    }

    private fun run(
        label: String,
        requireConnection: Boolean = false,
        allowDuringHandoff: Boolean = false,
        requireBluetooth: Boolean = true,
        requireReady: Boolean = true,
        action: () -> Unit,
    ) = postMain {
        try {
            check(!stopping.get()) { "耳机服务正在停止" }
            check(SessionState.operation == null) { "请等待当前操作完成" }
            check(allowDuringHandoff || SessionState.handoff?.active != true) { "交接仍在进行, 请等待完成或退出交接组" }
            check(!requireBluetooth || SessionState.permissionsGranted) { "请先授予蓝牙权限" }
            check(!requireReady || SessionState.ready) { "耳机服务尚未就绪, 请启动或重试" }
            check(!requireConnection || SessionState.connected) { "请先连接耳机控制通道" }
            SessionState.operation = label
            SessionState.record(label)
            worker.execute {
                try {
                    if (stopping.get()) throw SessionStopping()
                    onMain {
                        check(!requireBluetooth || SessionState.permissionsGranted) { "蓝牙权限已撤销" }
                        check(!requireReady || SessionState.ready) { "耳机服务已停止, 请先重新启动" }
                        check(!requireConnection || SessionState.connected) { "耳机控制通道已断开" }
                        check(allowDuringHandoff || SessionState.handoff?.active != true) { "交接仍在进行, 请稍后重试" }
                    }
                    action()
                } catch (error: Throwable) {
                    if (error !is SessionStopping) report("${label}失败", error)
                } finally {
                    onMain {
                        if (!stopping.get()) {
                            SessionState.operation = null
                            if (startPending && SessionState.permissionsGranted && !userStopped) requestStart()
                        }
                    }
                }
            }
        } catch (error: Throwable) {
            SessionState.notify("无法执行操作", error.message.orEmpty(), true)
        }
    }

    private fun poll() {
        if (stopping.get()) return
        try {
            if (!onMain { SessionState.ready && SessionState.permissionsGranted }) return
            val snapshot = native.poll(pollTick++ % 8 == 0, onMain { SessionState.address })
            if (stopping.get()) return
            pollFailures = 0
            val reconnect = consume(snapshot)
            if (reconnect != null && !stopping.get()) {
                try {
                    connectOnWorker(reconnect, onMain {
                        SessionState.devices.firstOrNull { sameAddress(it.address, reconnect) }?.name.orEmpty()
                    })
                } catch (error: Throwable) {
                    if (error !is SessionStopping) report("交接后的控制连接失败", error)
                } finally {
                    onMain { if (!stopping.get()) SessionState.operation = null }
                }
            }
        } catch (error: Throwable) {
            pollFailures += 1
            Log.e(TAG, "状态轮询失败 ($pollFailures/3)", error)
            if (pollFailures == 1) report("状态同步失败", error)
            if (pollFailures >= 3) {
                pump?.cancel(false)
                pump = null
                closeNative()
                onMain {
                    resetHandoff()
                    SessionState.stop()
                    SessionState.notify("状态同步已停止", "连续 3 次同步失败, 可在设置中重新启动. ${error.message.orEmpty()}", true)
                }
            }
        }
    }

    private fun consume(snapshot: NativeSnapshot): String? = onMain {
        SessionState.applySnapshot(snapshot.events, snapshot.peers, snapshot.holding, snapshot.audio)
        val handoff = SessionState.handoff
        if (handoff?.kind == "failed") resetHandoff()
        if (handoff?.active == true && handoffStarted == null) handoffStarted = SystemClock.elapsedRealtime()
        if (handoff?.kind == "done") {
            claimDone = true
            if (claimTarget == null) {
                handoffStarted = null
                handoffDelayReported = false
            }
        }
        val target = claimTarget
        if (claimDone && target != null) {
            if (sameAddress(SessionState.holding, target)) {
                SessionState.setHandoff("done")
                if (SessionState.operation == null && SessionState.permissionsGranted && !stopping.get()) {
                    resetHandoff()
                    if (!SessionState.connected || !sameAddress(SessionState.address, target)) {
                        SessionState.operation = "正在连接交接后的控制通道"
                        SessionState.record("正在连接交接后的控制通道")
                        return@onMain target
                    }
                }
            } else {
                SessionState.setHandoff("connecting", "交接流程已返回, 等待系统确认本机音频连接")
            }
        }
        val started = handoffStarted
        if (started != null && !handoffDelayReported && SystemClock.elapsedRealtime() - started > 60_000) {
            handoffDelayReported = true
            SessionState.notify("交接仍在处理中", "系统蓝牙操作或恢复尚未完成, 请等待结果. 当前操作结束前不能再次接管.")
        }
        null
    }

    private fun closeNative() {
        try {
            native.close()
        } catch (error: Throwable) {
            report("关闭耳机核心失败", error)
        } finally {
            try {
                BluetoothBridge.stop()
            } catch (error: Throwable) {
                report("释放蓝牙资源失败", error)
            }
        }
    }

    private fun resetHandoff() {
        claimTarget = null
        handoffStarted = null
        handoffDelayReported = false
        claimDone = false
    }

    private fun ensureUsable() {
        if (stopping.get()) throw SessionStopping()
        check(onMain { SessionState.permissionsGranted }) { "请先授予蓝牙权限" }
    }

    private fun requireGroup() = check(SessionState.groupJoined) { "请先加入交接组" }
    private fun requireAddress(value: String?): String = normalizeAddress(value)
        ?: error("蓝牙地址必须为 12 位十六进制数字, 例如 AA:BB:CC:DD:EE:FF")
    private fun validateKind(kind: String) = require(kind == "rfcomm" || kind == "ble") { "连接类型必须为 rfcomm 或 ble" }

    private fun report(title: String, error: Throwable) {
        Log.e(TAG, title, error)
        onMain { SessionState.notify(title, error.message ?: error.javaClass.simpleName, true) }
    }

    private fun postMain(action: () -> Unit) {
        if (Looper.myLooper() == Looper.getMainLooper()) action() else main.post { action() }
    }

    private fun <T> onMain(action: () -> T): T {
        if (Looper.myLooper() == Looper.getMainLooper()) return action()
        val task = FutureTask(Callable { action() })
        check(main.post(task)) { "无法将会话状态送回主线程" }
        return try {
            task.get()
        } catch (error: ExecutionException) {
            throw error.cause ?: error
        }
    }

    private class SessionStopping : RuntimeException()
}
