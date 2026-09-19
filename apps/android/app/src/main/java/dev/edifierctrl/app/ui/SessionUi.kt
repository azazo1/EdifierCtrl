package dev.edifierctrl.app.ui

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import org.json.JSONObject

/** 跨页面的连接 / 组 / 电量状态, 文案给用户看. */
object SessionUi {
    var connected by mutableStateOf(false)
    var address by mutableStateOf<String?>(null)
    var deviceName by mutableStateOf<String?>(null)
    var battery by mutableStateOf<Int?>(null)
    var hint by mutableStateOf("扫描已配对的耳机, 点卡片连接.")
    var groupJoined by mutableStateOf(false)
    var holding by mutableStateOf<String?>(null)
    var noise by mutableStateOf<String?>(null)

    fun statusLine(): String {
        val name = deviceName?.ifBlank { null } ?: address ?: "未连接耳机"
        val bat = battery?.let { "  ·  电量 $it%" } ?: ""
        val link = if (connected) "已连接" else "未连接"
        val hold = holding?.let { "  ·  持有 $it" } ?: ""
        val group = if (groupJoined) "  ·  已入组" else ""
        return "$link  $name$bat$hold$group"
    }

    fun applyEvent(raw: String) {
        val o = runCatching { JSONObject(raw) }.getOrNull() ?: return
        when (o.optString("kind")) {
            "empty" -> return
            "bt_state" -> {
                connected = o.optBoolean("connected")
                val addr = o.optString("address")
                if (addr.isNotEmpty()) {
                    address = addr
                }
                hint = if (connected) "控制通道已连接" else "控制通道已断开"
            }
            "headset" -> {
                val n = o.optJSONObject("notification") ?: return
                when (n.optString("kind")) {
                    "battery" -> {
                        battery = n.optInt("percent")
                        hint = "电量 ${battery}%"
                    }
                    "noise" -> {
                        noise = n.optString("mode")
                        hint = "降噪 ${noiseLabel(noise)}"
                    }
                    "name" -> {
                        deviceName = n.optString("name")
                        hint = "耳机 $deviceName"
                    }
                    else -> Unit
                }
            }
            "handoff" -> {
                val p = o.optJSONObject("progress") ?: return
                hint = when (p.optString("kind")) {
                    "requesting" -> "正在请求交接"
                    "waiting_peer" -> "等待对端释放音频"
                    "releasing" -> "正在释放音频"
                    "connecting" -> "正在接管音频"
                    "fallback_cd" -> "对端未释放, 已发 CD"
                    "done" -> "交接完成"
                    "failed" -> "交接失败: ${p.optString("reason")}"
                    "busy" -> "交接忙, 请稍后再试"
                    else -> hint
                }
            }
            "audio" -> {
                hint = when (o.optString("state")) {
                    "connected" -> "系统音频已连接"
                    "disconnected" -> "系统音频已断开"
                    "connecting" -> "正在连接系统音频"
                    else -> hint
                }
            }
            "message" -> {
                val text = o.optString("text")
                if (text.isNotEmpty()) {
                    hint = text
                }
            }
        }
    }
}

fun noiseLabel(mode: String?): String = when (mode) {
    "normal" -> "关闭"
    "reduction" -> "降噪"
    "ambient" -> "通透"
    else -> mode ?: "未知"
}
