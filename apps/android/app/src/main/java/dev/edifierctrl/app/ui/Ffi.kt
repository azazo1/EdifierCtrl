package dev.edifierctrl.app.ui

import android.content.Context
import dev.edifierctrl.app.BluetoothBridge
import dev.edifierctrl.app.EdifierNative
import org.json.JSONArray

internal fun encode(json: String): String {
    if (!EdifierNative.loaded) {
        return "尚未加载 libedifier_ffi.so"
    }
    return EdifierNative.commandEncode(json) ?: (EdifierNative.lastError() ?: "编码失败")
}

internal fun sendJson(session: Long, json: String): String {
    if (!EdifierNative.loaded || session == 0L) {
        return encode(json)
    }
    val rc = runCatching { EdifierNative.sessionSendJson(session, json) }.getOrDefault(-1)
    return if (rc == 0) "已发送" else encode(json)
}

internal fun nativeCall(block: () -> String?): String {
    if (!EdifierNative.loaded) {
        return "尚未加载 libedifier_ffi.so"
    }
    return runCatching { block() ?: (EdifierNative.lastError() ?: "") }.getOrElse { it.message ?: "FFI 失败" }
}

internal fun joinGroup(session: Long, pass: String): String {
    return nativeCall {
        val rc = EdifierNative.sessionGroupJoin(session, pass)
        if (rc != 0) {
            EdifierNative.lastError()
        } else {
            SessionUi.groupJoined = true
            SessionUi.holding = EdifierNative.sessionHolding(session)
            SessionUi.peers = loadPeers(session)
            "已加入组"
        }
    }
}

internal data class DeviceRow(val address: String, val name: String)

internal fun parseDevices(json: String): List<DeviceRow> {
    return runCatching {
        val arr = JSONArray(json)
        (0 until arr.length()).map { i ->
            val o = arr.getJSONObject(i)
            DeviceRow(o.optString("address"), o.optString("name"))
        }
    }.getOrDefault(emptyList())
}

internal fun savedPassphrase(context: Context): String {
    return context.getSharedPreferences("edifierctrl", Context.MODE_PRIVATE).getString("passphrase", "").orEmpty()
}

internal fun savePassphrase(context: Context, pass: String) {
    context.getSharedPreferences("edifierctrl", Context.MODE_PRIVATE).edit().putString("passphrase", pass).apply()
}

internal fun autoConnectLinked(session: Long) {
    if (SessionUi.connected) {
        return
    }
    val list = parseDevices(BluetoothBridge.connectedEdifier())
    if (list.isEmpty()) {
        return
    }
    val pick = list.firstOrNull { sameMac(it.address, SessionUi.holding) } ?: list.first()
    val rc = EdifierNative.sessionConnect(session, pick.address, "rfcomm")
    if (rc != 0) {
        return
    }
    SessionUi.address = pick.address
    SessionUi.deviceName = pick.name.ifBlank { pick.address }
    SessionUi.connected = true
    EdifierNative.sessionSendJson(session, """{"op":"query_battery"}""")
    SessionUi.hint = "已自动连接 ${pick.name.ifBlank { pick.address }}"
}

internal fun loadPeers(session: Long): List<GroupPeer> {
    val json = nativeCall { EdifierNative.sessionGroupPeers(session) }
    return runCatching {
        val arr = JSONArray(json)
        val map = linkedMapOf<String, GroupPeer>()
        for (i in 0 until arr.length()) {
            val o = arr.getJSONObject(i)
            val id = o.optString("id")
            val host = o.optString("hostname").ifBlank { id }
            val holding = if (o.isNull("holding")) null else o.optString("holding").ifBlank { null }
            map[id] = GroupPeer(id, host, holding)
        }
        map.values.toList()
    }.getOrDefault(emptyList())
}
