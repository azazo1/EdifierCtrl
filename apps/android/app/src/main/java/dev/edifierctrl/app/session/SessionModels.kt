package dev.edifierctrl.app.session

import java.util.Locale

/** 在 Compose 页面与应用会话 worker 之间共享的不可变数据. */
data class HeadphoneProfile(
    val id: String,
    val displayName: String,
    val capabilities: Set<String>,
    val maxNameLen: Int,
    val serviceUuid: String?,
) {
    fun supports(feature: String): Boolean = capabilities.contains(feature)
}

data class HeadphoneDevice(
    val address: String,
    val name: String,
    val kind: String = "rfcomm",
    val serviceUuid: String? = null,
) {
    val displayName: String
        get() = name.ifBlank { address }
}

data class GroupPeer(
    val id: String,
    val host: String,
    val holding: String?,
    val platform: String,
    val appVersion: String,
    val canAudio: Boolean,
) {
    val displayName: String
        get() = host.ifBlank { id }
}

data class ActivityEntry(
    val id: Long,
    val time: Long,
    val title: String,
    val detail: String,
    val isError: Boolean,
)

data class UserNotice(
    val title: String,
    val detail: String,
    val isError: Boolean,
)

data class HandoffProgress(
    val kind: String,
    val title: String,
    val detail: String,
    val step: Int,
    val active: Boolean,
)

fun normalizeAddress(value: String?): String? {
    if (value.isNullOrBlank()) return null
    val compact = value.trim().replace(":", "").replace("-", "")
    if (compact.length != 12 || compact.any { it !in '0'..'9' && it !in 'a'..'f' && it !in 'A'..'F' }) return null
    return compact.uppercase(Locale.US).chunked(2).joinToString(":")
}

fun sameAddress(a: String?, b: String?): Boolean =
    normalizeAddress(a) != null && normalizeAddress(a) == normalizeAddress(b)

fun noiseLabel(value: String?): String = when (value) {
    "normal" -> "关闭"
    "reduction" -> "降噪"
    "ambient" -> "通透"
    else -> value ?: "未知"
}

fun effectLabel(value: String?): String = when (value) {
    "normal" -> "标准"
    "pop" -> "流行"
    "classical" -> "古典"
    "rock" -> "摇滚"
    else -> value ?: "未知"
}

fun ldacLabel(value: String?): String = when (value) {
    "off" -> "关闭"
    "rate48k" -> "44.1k / 48k"
    "rate96k" -> "96k"
    else -> value ?: "未知"
}
