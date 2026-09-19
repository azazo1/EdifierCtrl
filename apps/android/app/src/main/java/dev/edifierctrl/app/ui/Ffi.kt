package dev.edifierctrl.app.ui

import dev.edifierctrl.app.EdifierNative

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
