package dev.edifierctrl.app.session

import dev.edifierctrl.app.BluetoothBridge
import dev.edifierctrl.app.EdifierNative
import org.json.JSONArray
import org.json.JSONObject

internal data class NativeSnapshot(
    val events: List<String>,
    val peers: List<GroupPeer>?,
    val holding: String?,
    val audio: String,
)

/** 句柄和 JNI 错误都只在同一个应用 worker 线程上读写. */
internal class NativeSession {
    private var owner: Thread? = null
    private var handle = 0L
    private var joined = false

    fun prepare(localId: String): Pair<String, List<HeadphoneProfile>> {
        assertWorker()
        check(EdifierNative.ensureLoaded()) { EdifierNative.loadError ?: "无法加载耳机核心" }
        if (handle == 0L) {
            handle = EdifierNative.sessionNew(localId)
            check(handle != 0L) { failureText() }
        }
        val version = EdifierNative.version()
        val source = JSONArray(text(EdifierNative.profilesJson()))
        val profiles = (0 until source.length()).map { index ->
            val item = source.getJSONObject(index)
            val features = item.getJSONArray("features")
            HeadphoneProfile(
                id = item.getString("id"),
                displayName = item.getString("display_name"),
                capabilities = (0 until features.length()).map { features.getString(it) }.toSet(),
                maxNameLen = item.getInt("max_name_len"),
                serviceUuid = item.optionalText("unique_service_uuid"),
            )
        }
        require(profiles.isNotEmpty() && profiles.all { it.id.isNotBlank() && it.maxNameLen > 0 }) {
            "核心机型档案缺少有效的机型或名称长度"
        }
        return version to profiles
    }

    fun scan(kind: String): List<HeadphoneDevice> = parseDevices(text(EdifierNative.sessionScan(session(), kind)))

    fun connect(address: String, kind: String) = checkResult(EdifierNative.sessionConnect(session(), address, kind))
    fun disconnect() = checkResult(EdifierNative.sessionDisconnect(session()))
    fun readout(profile: String) = checkResult(EdifierNative.sessionReadout(session(), profile))
    fun send(command: String) = checkResult(EdifierNative.sessionSendJson(session(), command))

    fun join(group: String) {
        checkResult(EdifierNative.sessionGroupJoin(session(), group))
        joined = true
    }

    fun leave() {
        checkResult(EdifierNative.sessionGroupLeave(session()))
        joined = false
    }

    fun claim(address: String) = checkResult(EdifierNative.sessionGroupClaim(session(), address))
    fun claimPeer(id: String) = checkResult(EdifierNative.sessionGroupClaimPeer(session(), id))

    fun poll(includePeers: Boolean, address: String?): NativeSnapshot {
        val session = session()
        val events = mutableListOf<String>()
        for (index in 0 until 64) {
            val raw = text(EdifierNative.sessionPollEvent(session))
            if (JSONObject(raw).getString("kind") == "empty") break
            events.add(raw)
        }
        val peers = if (joined && includePeers) parsePeers(text(EdifierNative.sessionGroupPeers(session))) else null
        val holding = normalizeAddress(text(EdifierNative.sessionHolding(session)))
        val audioAddress = normalizeAddress(address) ?: holding
        val audio = if (audioAddress == null) "unknown" else BluetoothBridge.audioState(audioAddress)
        return NativeSnapshot(events.toList(), peers, holding, audio)
    }

    fun diagnostic(input: String, parse: Boolean): String {
        assertWorker()
        check(EdifierNative.ensureLoaded()) { EdifierNative.loadError ?: "无法加载耳机核心" }
        val value = text(if (parse) EdifierNative.frameParse(input) else EdifierNative.commandEncode(input))
        return JSONObject(value).toString(2)
    }

    fun close() {
        assertWorker()
        val current = handle
        if (current == 0L) return
        var failure: Throwable? = null
        try {
            if (joined) {
                try {
                    checkResult(EdifierNative.sessionGroupLeave(current))
                } catch (error: Throwable) {
                    failure = error
                }
            }
            try {
                checkResult(EdifierNative.sessionDisconnect(current))
            } catch (error: Throwable) {
                if (failure == null) failure = error else failure.addSuppressed(error)
            }
        } finally {
            try {
                EdifierNative.sessionFree(current)
            } finally {
                handle = 0L
                joined = false
            }
        }
        failure?.let { throw it }
    }

    private fun session(): Long {
        assertWorker()
        check(handle != 0L) { "耳机服务尚未启动" }
        return handle
    }

    private fun checkResult(result: Int) {
        if (result != 0) error(failureText())
    }

    private fun text(value: String?): String = value ?: error(failureText())

    private fun failureText(): String = EdifierNative.lastError()?.takeIf { it.isNotBlank() }
        ?: "耳机核心未返回详细错误"

    private fun assertWorker() {
        val current = Thread.currentThread()
        if (owner == null) owner = current
        check(owner === current) { "JNI 必须由同一个会话 worker 串行调用" }
    }

    companion object {
        fun parseDevices(json: String): List<HeadphoneDevice> {
            val source = JSONArray(json)
            return (0 until source.length()).mapNotNull { index ->
                val item = source.getJSONObject(index)
                val address = normalizeAddress(item.optionalText("address")) ?: return@mapNotNull null
                HeadphoneDevice(
                    address = address,
                    name = item.optionalText("name").orEmpty(),
                    kind = item.optionalText("kind") ?: "rfcomm",
                    serviceUuid = item.optionalText("service_uuid"),
                )
            }.distinctBy { it.address }
        }

        private fun parsePeers(json: String): List<GroupPeer> {
            val source = JSONArray(json)
            return (0 until source.length()).map { index ->
                val item = source.getJSONObject(index)
                GroupPeer(
                    id = item.getString("id"),
                    host = item.optionalText("hostname").orEmpty(),
                    holding = normalizeAddress(item.optionalText("holding")),
                    platform = item.optionalText("os").orEmpty(),
                    appVersion = item.optionalText("app_version").orEmpty(),
                    canAudio = item.optBoolean("can_audio", false),
                )
            }
        }

        private fun JSONObject.optionalText(key: String): String? =
            if (!has(key) || isNull(key)) null else getString(key).takeIf { it.isNotBlank() }
    }
}
