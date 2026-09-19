package dev.edifierctrl.app.session

import org.json.JSONObject
import java.nio.CharBuffer
import java.nio.charset.CodingErrorAction

/** 发送前按档案和协议检查参数, 查询与写入共用功能校验. */
internal object CommandValidation {
    fun build(op: String, values: JSONObject): JSONObject {
        require(op.isNotBlank()) { "指令名称不能为空" }
        return JSONObject(values.toString()).put("op", op)
    }

    fun validate(command: JSONObject, profile: HeadphoneProfile) {
        val op = command.getString("op")
        val capability = when (op) {
            "set_noise_mode", "query_noise" -> "noise"
            "set_ambient_volume" -> "ambient_sound"
            "set_sound_effect", "query_sound_effect" -> "sound_effect"
            "set_control_settings", "query_control_settings" -> "control_settings"
            "set_ldac", "query_ldac" -> "ldac"
            "set_game_mode", "query_game_mode" -> "game_mode"
            "set_auto_power_off", "query_auto_power_off" -> "auto_power_off"
            "set_prompt_volume", "query_prompt_volume" -> "prompt_volume"
            "set_shutdown_timer", "disable_shutdown_timer", "query_shutdown_timer" -> "shutdown_timer"
            "set_name", "query_name" -> "name"
            "playback", "query_playback" -> "playback"
            else -> null
        }
        require(capability == null || profile.supports(capability)) { "当前机型不支持这项功能" }
        if (op == "set_noise_mode" && command.optString("mode") == "ambient") {
            require(profile.supports("ambient_sound")) { "当前机型不支持通透模式" }
        }
        if (op == "set_name") {
            val name = command.opt("name") as? String ?: error("请输入有效的耳机名称")
            require('\u0000' !in name) { "耳机名称不能包含空字符" }
            val encoder = Charsets.UTF_8.newEncoder()
                .onMalformedInput(CodingErrorAction.REPORT)
                .onUnmappableCharacter(CodingErrorAction.REPORT)
            require(encoder.encode(CharBuffer.wrap(name)).remaining() <= profile.maxNameLen) {
                "当前机型名称最多支持 ${profile.maxNameLen} 个 UTF-8 字节"
            }
        }
        if (op == "set_control_settings") {
            val modes = listOf("normal", "reduction", "ambient")
            require(modes.all { command.opt(it) is Boolean }) { "请选择有效的按键切换模式" }
            require(modes.count { command.getBoolean(it) } >= 2) { "请选择至少两种模式, 耳机按键需要循环切换" }
        }
    }
}
