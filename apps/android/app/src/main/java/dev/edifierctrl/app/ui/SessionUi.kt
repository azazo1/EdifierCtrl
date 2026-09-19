package dev.edifierctrl.app.ui

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import org.json.JSONObject

/** 跨页面的连接 / 组 / 电量状态, 文案给用户看. */
object SessionUi {
    var connected by mutableStateOf(false)
    var address by mutableStateOf<String?>(null)
    var deviceName by mutableStateOf<String?>(null)
    var battery by mutableStateOf<Int?>(null)
    var mac by mutableStateOf<String?>(null)
    var firmware by mutableStateOf<String?>(null)
    var hint by mutableStateOf("扫描已配对的耳机, 点卡片连接.")
    var groupJoined by mutableStateOf(false)
    var holding by mutableStateOf<String?>(null)
    var noise by mutableStateOf<String?>(null)
    var ambientVolume by mutableIntStateOf(0)
    var effect by mutableStateOf<String?>(null)
    var gameMode by mutableStateOf(false)
    var ldac by mutableStateOf<String?>(null)
    var promptVolume by mutableIntStateOf(7)
    var shutdownOn by mutableStateOf(false)
    var shutdownMinutes by mutableIntStateOf(5)
    var autoPowerOff by mutableStateOf(false)
    var controlNormal by mutableStateOf(true)
    var controlReduction by mutableStateOf(true)
    var controlAmbient by mutableStateOf(true)

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
            "headset" -> applyHeadset(o.optJSONObject("notification") ?: return)
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

    private fun applyHeadset(n: JSONObject) {
        when (n.optString("kind")) {
            "battery" -> {
                battery = n.optInt("percent")
                hint = "电量 ${battery}%"
            }
            "noise" -> {
                noise = n.optString("mode")
                if (n.has("ambient_volume")) {
                    ambientVolume = n.optInt("ambient_volume")
                }
                hint = "降噪 ${noiseLabel(noise)}"
            }
            "name" -> {
                deviceName = n.optString("name")
                hint = "耳机 $deviceName"
            }
            "mac" -> {
                mac = n.optString("address")
                hint = "MAC $mac"
            }
            "firmware" -> {
                firmware = n.optString("version")
                hint = "固件 $firmware"
            }
            "sound_effect" -> {
                effect = n.optString("effect")
                hint = "音效 ${effectLabel(effect)}"
            }
            "game_mode" -> {
                gameMode = n.optBoolean("on")
                hint = if (gameMode) "游戏模式开" else "游戏模式关"
            }
            "ldac" -> {
                ldac = n.optString("mode")
                hint = "LDAC ${ldacLabel(ldac)}"
            }
            "prompt_volume" -> {
                promptVolume = n.optInt("volume")
                hint = "提示音量 $promptVolume"
            }
            "shutdown_timer_enabled" -> {
                shutdownOn = n.optBoolean("on")
                hint = if (shutdownOn) "定时关机开" else "定时关机关"
            }
            "shutdown_timer" -> {
                shutdownOn = true
                shutdownMinutes = n.optInt("minutes").coerceIn(1, 180)
                hint = "定时关机 ${shutdownMinutes} 分钟"
            }
            "auto_power_off" -> {
                autoPowerOff = n.optBoolean("on")
                hint = if (autoPowerOff) "自动关机开" else "自动关机关"
            }
            "control_settings" -> {
                controlNormal = n.optBoolean("normal")
                controlReduction = n.optBoolean("reduction")
                controlAmbient = n.optBoolean("ambient")
                hint = "按键可切换模式已更新"
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

fun effectLabel(effect: String?): String = when (effect) {
    "normal" -> "标准"
    "pop" -> "流行"
    "classical" -> "古典"
    "rock" -> "摇滚"
    else -> effect ?: "未知"
}

fun ldacLabel(mode: String?): String = when (mode) {
    "off" -> "关闭"
    "rate48k" -> "44.1k / 48k"
    "rate96k" -> "96k"
    else -> mode ?: "未知"
}
