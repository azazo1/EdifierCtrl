package dev.edifierctrl.app.session

import android.os.Looper
import android.util.Log
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import org.json.JSONObject

/** 跨页面共享的已确认状态, 只能在主线程归并. */
object SessionState {
    private const val TAG = "EdifierSession"
    private var nextActivityId = 0L
    private var scannedDevices: List<HeadphoneDevice> = emptyList()
    var audio by mutableStateOf("unknown")
        internal set

    var ready by mutableStateOf(false)
        internal set
    var permissionsGranted by mutableStateOf(false)
        internal set
    var nativeAvailable by mutableStateOf(false)
        internal set
    var operation by mutableStateOf<String?>(null)
        internal set
    val busy: Boolean
        get() = operation != null || handoff?.active == true
    val canControl: Boolean
        get() = ready && permissionsGranted && connected && !busy

    var connected by mutableStateOf(false)
        internal set
    var address by mutableStateOf<String?>(null)
        internal set
    var deviceName by mutableStateOf<String?>(null)
        internal set
    var holding by mutableStateOf<String?>(null)
        internal set
    val headphoneName: String
        get() = deviceName?.takeIf { it.isNotBlank() }
            ?: devices.firstOrNull { sameAddress(it.address, address) }?.displayName
            ?: "你的漫步者耳机"
    val audioLabel: String
        get() = when (audio) {
            "connected" -> "系统音频已就绪"
            "connecting" -> "系统音频连接中"
            "disconnected" -> "系统音频未连接"
            else -> "系统音频待确认"
        }

    var coreVersion by mutableStateOf("")
        internal set
    var devices by mutableStateOf<List<HeadphoneDevice>>(emptyList())
        internal set
    var profiles by mutableStateOf<List<HeadphoneProfile>>(emptyList())
        internal set
    var peers by mutableStateOf<List<GroupPeer>>(emptyList())
        internal set
    var activities by mutableStateOf<List<ActivityEntry>>(emptyList())
        internal set
    var selectedProfile by mutableStateOf(
        HeadphoneProfile("basedevice", "尚未加载机型", emptySet(), 24, null),
    )
        internal set

    var groupJoined by mutableStateOf(false)
        internal set
    var joinedGroupName by mutableStateOf("")
        internal set
    var handoff by mutableStateOf<HandoffProgress?>(null)
        internal set
    var notice by mutableStateOf<UserNotice?>(null)
        internal set

    var battery by mutableStateOf<Int?>(null)
        internal set
    var ambientVolume by mutableStateOf<Int?>(null)
        internal set
    var promptVolume by mutableStateOf<Int?>(null)
        internal set
    var shutdownMinutes by mutableStateOf<Int?>(null)
        internal set
    var gameMode by mutableStateOf<Boolean?>(null)
        internal set
    var shutdownOn by mutableStateOf<Boolean?>(null)
        internal set
    var autoPowerOff by mutableStateOf<Boolean?>(null)
        internal set
    var controlNormal by mutableStateOf<Boolean?>(null)
        internal set
    var controlReduction by mutableStateOf<Boolean?>(null)
        internal set
    var controlAmbient by mutableStateOf<Boolean?>(null)
        internal set
    var noise by mutableStateOf<String?>(null)
        internal set
    var effect by mutableStateOf<String?>(null)
        internal set
    var ldac by mutableStateOf<String?>(null)
        internal set
    var mac by mutableStateOf<String?>(null)
        internal set
    var firmware by mutableStateOf<String?>(null)
        internal set
    var diagnosticOutput by mutableStateOf("")
        internal set

    fun supports(feature: String): Boolean = selectedProfile.supports(feature)

    internal fun clearNotice() {
        assertMain()
        notice = null
    }

    internal fun clearActivities() {
        assertMain()
        activities = emptyList()
    }

    internal fun initialize(
        version: String,
        availableProfiles: List<HeadphoneProfile>,
        selectedId: String,
        lastAddress: String?,
        lastName: String?,
    ) {
        assertMain()
        require(availableProfiles.isNotEmpty()) { "核心未返回机型档案" }
        coreVersion = version
        profiles = availableProfiles.toList()
        selectedProfile = availableProfiles.firstOrNull { it.id == selectedId } ?: availableProfiles.first()
        address = normalizeAddress(lastAddress)
        deviceName = lastName?.takeIf { it.isNotBlank() }
        ready = true
        record("耳机服务已启动", "核心版本 $version")
        rebuildDevices()
    }

    internal fun selectProfile(profile: HeadphoneProfile) {
        assertMain()
        selectedProfile = profile
        clearReadings()
    }

    internal fun updateDevices(value: List<HeadphoneDevice>) {
        assertMain()
        scannedDevices = value.toList()
        rebuildDevices()
    }

    internal fun setConnected(value: Boolean, nextAddress: String? = null, nextName: String? = null) {
        assertMain()
        val normalized = normalizeAddress(nextAddress)
        if (!value || (normalized != null && !sameAddress(address, normalized))) clearReadings()
        if (normalized != null) {
            if (!sameAddress(address, normalized)) deviceName = null
            address = normalized
        }
        if (!nextName.isNullOrBlank()) deviceName = nextName
        connected = value
        rebuildDevices()
    }

    internal fun setGroup(value: Boolean, name: String = "") {
        assertMain()
        groupJoined = value
        joinedGroupName = if (value) name else ""
        if (!value) {
            peers = emptyList()
            holding = null
            setHandoff(null)
        }
        rebuildDevices()
    }

    internal fun updatePeers(value: List<GroupPeer>) {
        assertMain()
        peers = value.filter { it.id.isNotBlank() }.map { peer ->
            peer.copy(holding = normalizeAddress(peer.holding))
        }.distinctBy { it.id }.sortedBy { it.displayName.lowercase() }
        rebuildDevices()
    }

    internal fun setDiagnostic(value: String) {
        assertMain()
        diagnosticOutput = value
    }

    internal fun stop() {
        assertMain()
        ready = false
        connected = false
        operation = null
        clearReadings()
        setGroup(false)
        address = null
        deviceName = null
        audio = "unknown"
        record("耳机服务已停止")
        rebuildDevices()
    }

    internal fun notify(title: String, detail: String, error: Boolean = false) {
        assertMain()
        notice = UserNotice(title, detail, error)
        record(title, detail, error)
    }

    internal fun record(title: String, detail: String = "", error: Boolean = false) {
        assertMain()
        val entry = ActivityEntry(
            id = ++nextActivityId,
            time = System.currentTimeMillis(),
            title = title,
            detail = detail,
            isError = error,
        )
        activities = (listOf(entry) + activities).take(200)
        val message = if (detail.isBlank()) title else "$title: $detail"
        if (error) Log.e(TAG, message) else Log.i(TAG, message)
    }

    internal fun setHandoff(kind: String?, reason: String? = null) {
        assertMain()
        handoff = kind?.let {
            HandoffProgress(
                kind = it,
                title = handoffTitle(it),
                detail = reason ?: handoffDetail(it),
                step = handoffStep(it),
                active = it in ACTIVE_HANDOFF,
            )
        }
    }

    internal fun applySnapshot(
        events: List<String>,
        nextPeers: List<GroupPeer>?,
        nextHolding: String?,
        nextAudio: String,
    ) {
        assertMain()
        if (groupJoined && nextPeers != null) updatePeers(nextPeers)
        events.forEach(::applyEvent)
        holding = normalizeAddress(nextHolding)
        audio = nextAudio.takeIf { it in AUDIO_STATES } ?: "unknown"
        rebuildDevices()
    }

    private fun applyEvent(raw: String) {
        val root = runCatching { JSONObject(raw) }.getOrNull() ?: return
        when (root.optString("kind")) {
            "bt_state" -> {
                val value = root.optBoolean("connected", false)
                setConnected(value, nullableString(root, "address"))
                record(if (value) "控制通道已连接" else "控制通道已断开")
            }
            "headset" -> if (connected) root.optJSONObject("notification")?.let(::applyHeadset)
            "audio" -> {
                val value = root.optString("state").takeIf { it in AUDIO_STATES } ?: "unknown"
                audio = value
                if (value == "disconnected") holding = null
            }
            "handoff" -> {
                if (!groupJoined) return
                val progress = root.optJSONObject("progress") ?: return
                val kind = progress.optString("kind").takeIf { it.isNotBlank() } ?: return
                val reason = nullableString(progress, "reason")
                if (kind == "busy") {
                    notify("已有交接正在进行", reason ?: "等待当前交接完成后再试", true)
                } else {
                    setHandoff(kind, reason)
                    val current = handoff
                    if (current != null) record(current.title, current.detail, kind == "failed")
                    if (kind == "failed") notify(current?.title ?: "交接未完成", current?.detail.orEmpty(), true)
                }
            }
            "message" -> nullableString(root, "text")?.takeIf { it.isNotEmpty() }?.let {
                record("耳机服务", it)
            }
        }
    }

    private fun applyHeadset(notification: JSONObject) {
        when (notification.optString("kind")) {
            "battery" -> battery = nullableInt(notification, "percent")?.takeIf { it in 0..100 }
            "noise" -> {
                noise = nullableString(notification, "mode")
                ambientVolume = nullableInt(notification, "ambient_volume")
            }
            "name" -> deviceName = nullableString(notification, "name")
            "mac" -> mac = normalizeAddress(nullableString(notification, "address"))
            "firmware" -> firmware = nullableString(notification, "version")
            "sound_effect" -> effect = nullableString(notification, "effect")
            "game_mode" -> gameMode = nullableBoolean(notification, "on")
            "ldac" -> ldac = nullableString(notification, "mode")
            "prompt_volume" -> promptVolume = nullableInt(notification, "volume")
            "shutdown_timer_enabled" -> shutdownOn = nullableBoolean(notification, "on")
            "shutdown_timer" -> {
                shutdownMinutes = nullableInt(notification, "minutes")
                shutdownOn = shutdownMinutes?.let { true }
            }
            "auto_power_off" -> autoPowerOff = nullableBoolean(notification, "on")
            "control_settings" -> {
                controlNormal = nullableBoolean(notification, "normal")
                controlReduction = nullableBoolean(notification, "reduction")
                controlAmbient = nullableBoolean(notification, "ambient")
            }
        }
    }

    internal fun clearReadings() {
        assertMain()
        battery = null
        ambientVolume = null
        promptVolume = null
        shutdownMinutes = null
        gameMode = null
        shutdownOn = null
        autoPowerOff = null
        controlNormal = null
        controlReduction = null
        controlAmbient = null
        noise = null
        effect = null
        ldac = null
        mac = null
        firmware = null
    }

    private fun rebuildDevices() {
        val map = linkedMapOf<String, HeadphoneDevice>()
        scannedDevices.forEach { item -> normalizeAddress(item.address)?.let { map[it] = item.copy(address = it) } }
        peers.filter { it.canAudio && it.holding != null }.forEach { peer ->
            val address = peer.holding ?: return@forEach
            if (!sameAddress(holding, address)) {
                map.putIfAbsent(address, HeadphoneDevice(address, "${peer.displayName} 的耳机"))
            }
        }
        devices = map.values.sortedWith(compareBy<HeadphoneDevice, String>(String.CASE_INSENSITIVE_ORDER) { it.displayName }).toList()
    }

    private fun assertMain() {
        check(Looper.myLooper() == Looper.getMainLooper()) { "会话状态必须在主线程更新" }
    }

    private fun nullableString(value: JSONObject, key: String): String? =
        if (!value.has(key) || value.isNull(key)) null else value.optString(key).takeIf { it.isNotEmpty() }

    private fun nullableInt(value: JSONObject, key: String): Int? =
        if (!value.has(key) || value.isNull(key)) null else runCatching { value.getInt(key) }.getOrNull()

    private fun nullableBoolean(value: JSONObject, key: String): Boolean? =
        if (!value.has(key) || value.isNull(key)) null else runCatching { value.getBoolean(key) }.getOrNull()

    private fun handoffTitle(kind: String): String = when (kind) {
        "requesting" -> "正在请求交接"
        "waiting_peer" -> "等待另一台设备释放耳机"
        "releasing" -> "正在将耳机交给另一台设备"
        "connecting" -> "正在连接本机音频"
        "fallback_cd" -> "正在尝试释放原连接"
        "done" -> "交接已完成"
        "failed" -> "交接未完成"
        else -> "交接状态已更新"
    }

    private fun handoffDetail(kind: String): String = when (kind) {
        "requesting" -> "请求已经发出, 等待两端确认"
        "waiting_peer" -> "保持两端应用运行并等待音频释放"
        "releasing" -> "正在释放本机系统音频连接"
        "connecting" -> "等待系统确认音频连接"
        "fallback_cd" -> "原连接未释放, 正在尝试耳机断连指令"
        "done" -> "系统已确认交接结果"
        "failed" -> "请检查两端蓝牙状态后重试"
        else -> ""
    }

    private fun handoffStep(kind: String): Int = when (kind) {
        "waiting_peer", "releasing", "fallback_cd" -> 1
        "connecting" -> 2
        "done" -> 3
        else -> 0
    }

    private val ACTIVE_HANDOFF = setOf("requesting", "waiting_peer", "releasing", "connecting", "fallback_cd")
    private val AUDIO_STATES = setOf("unknown", "connected", "disconnected", "connecting")
}
