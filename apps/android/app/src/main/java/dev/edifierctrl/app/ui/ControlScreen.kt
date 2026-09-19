package dev.edifierctrl.app.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Slider
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import dev.edifierctrl.app.EdifierNative
import kotlin.math.roundToInt
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

private data class ConfirmOp(val title: String, val body: String, val json: String)

@OptIn(ExperimentalLayoutApi::class)
@Composable
fun ControlScreen() {
    var confirm by remember { mutableStateOf<ConfirmOp?>(null) }
    var nameDraft by remember { mutableStateOf(SessionUi.deviceName.orEmpty()) }
    val session = remember { EdifierNative.ensureSession() }
    val scope = rememberCoroutineScope()
    LaunchedEffect(SessionUi.deviceName) {
        if (nameDraft.isEmpty()) {
            nameDraft = SessionUi.deviceName.orEmpty()
        }
    }
    Column(
        Modifier
            .fillMaxSize()
            .padding(horizontal = 20.dp, vertical = 12.dp)
            .verticalScroll(rememberScrollState()),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Text("控制", style = MaterialTheme.typography.headlineSmall)
        Text(
            if (SessionUi.connected) "改设置会发到已连接的耳机." else "先到设备页连接耳机.",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        StatusBanner()
        Section("信息") {
            InfoLine("电量", SessionUi.battery?.let { "$it%" } ?: "-")
            InfoLine("MAC", SessionUi.mac ?: "-")
            InfoLine("固件", SessionUi.firmware ?: "-")
            FilledTonalButton(onClick = {
                SessionUi.hint = "正在读取状态"
                scope.launch {
                    SessionUi.hint = withContext(Dispatchers.IO) {
                        nativeCall {
                            val rc = EdifierNative.sessionReadout(session, "basedevice")
                            if (rc != 0) EdifierNative.lastError() else "已请求读取状态"
                        }
                    }
                }
            }) { Text("读取全部") }
        }
        Section("名称", "改完要删配对记录再配对才生效.") {
            OutlinedTextField(
                value = nameDraft,
                onValueChange = { nameDraft = it },
                label = { Text("耳机名") },
                modifier = Modifier.fillMaxWidth(),
                singleLine = true,
            )
            FilledTonalButton(onClick = {
                val escaped = nameDraft.replace("\\", "\\\\").replace("\"", "\\\"")
                SessionUi.hint = sendJson(session, """{"op":"set_name","name":"$escaped"}""")
            }) { Text("设置名称") }
        }
        Section("降噪") {
            ChipRow(
                options = listOf("normal" to "关闭", "reduction" to "降噪", "ambient" to "通透"),
                selected = SessionUi.noise,
            ) { mode ->
                SessionUi.noise = mode
                SessionUi.hint = sendJson(session, """{"op":"set_noise_mode","mode":"$mode"}""")
            }
        }
        Section("通透音量", "仅通透模式有效, -3 到 3.") {
            Text("${SessionUi.ambientVolume}")
            Slider(
                value = SessionUi.ambientVolume.toFloat(),
                onValueChange = { SessionUi.ambientVolume = it.roundToInt() },
                onValueChangeFinished = {
                    SessionUi.hint = sendJson(
                        session,
                        """{"op":"set_ambient_volume","volume":${SessionUi.ambientVolume}}""",
                    )
                },
                valueRange = -3f..3f,
                steps = 5,
            )
        }
        Section("按键可切换的模式", "至少选 2 项.") {
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                FilterChip(
                    selected = SessionUi.controlNormal,
                    onClick = {
                        SessionUi.controlNormal = !SessionUi.controlNormal
                        sendControl(session)
                    },
                    label = { Text("关闭") },
                )
                FilterChip(
                    selected = SessionUi.controlReduction,
                    onClick = {
                        SessionUi.controlReduction = !SessionUi.controlReduction
                        sendControl(session)
                    },
                    label = { Text("降噪") },
                )
                FilterChip(
                    selected = SessionUi.controlAmbient,
                    onClick = {
                        SessionUi.controlAmbient = !SessionUi.controlAmbient
                        sendControl(session)
                    },
                    label = { Text("通透") },
                )
            }
        }
        Section("音效") {
            ChipRow(
                options = listOf(
                    "normal" to "标准",
                    "pop" to "流行",
                    "classical" to "古典",
                    "rock" to "摇滚",
                ),
                selected = SessionUi.effect,
            ) { effect ->
                SessionUi.effect = effect
                SessionUi.hint = sendJson(session, """{"op":"set_sound_effect","effect":"$effect"}""")
            }
        }
        Section("提示音量") {
            Text("${SessionUi.promptVolume}")
            Slider(
                value = SessionUi.promptVolume.toFloat(),
                onValueChange = { SessionUi.promptVolume = it.roundToInt() },
                onValueChangeFinished = {
                    SessionUi.hint = sendJson(
                        session,
                        """{"op":"set_prompt_volume","volume":${SessionUi.promptVolume}}""",
                    )
                },
                valueRange = 0f..15f,
                steps = 14,
            )
        }
        Section("定时关机", "断电后会回到默认.") {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.SpaceBetween, modifier = Modifier.fillMaxWidth()) {
                Text(if (SessionUi.shutdownOn) "已开启" else "关闭")
                Switch(
                    checked = SessionUi.shutdownOn,
                    onCheckedChange = { on ->
                        SessionUi.shutdownOn = on
                        SessionUi.hint = if (on) {
                            sendJson(session, """{"op":"set_shutdown_timer","minutes":${SessionUi.shutdownMinutes}}""")
                        } else {
                            sendJson(session, """{"op":"disable_shutdown_timer"}""")
                        }
                    },
                )
            }
            Text("${SessionUi.shutdownMinutes} 分钟")
            Slider(
                value = SessionUi.shutdownMinutes.toFloat(),
                onValueChange = { SessionUi.shutdownMinutes = it.roundToInt().coerceIn(1, 180) },
                onValueChangeFinished = {
                    if (SessionUi.shutdownOn) {
                        SessionUi.hint = sendJson(
                            session,
                            """{"op":"set_shutdown_timer","minutes":${SessionUi.shutdownMinutes}}""",
                        )
                    }
                },
                valueRange = 1f..180f,
                steps = 178,
                enabled = SessionUi.shutdownOn,
            )
        }
        Section("LDAC", "改完要重新配对才生效, 同时会关掉定时关机.") {
            ChipRow(
                options = listOf("off" to "关闭", "rate48k" to "44.1k / 48k", "rate96k" to "96k"),
                selected = SessionUi.ldac,
            ) { mode ->
                SessionUi.ldac = mode
                SessionUi.hint = sendJson(session, """{"op":"set_ldac","mode":"$mode"}""")
            }
        }
        Section("开关") {
            ToggleRow("游戏模式", SessionUi.gameMode) { on ->
                SessionUi.gameMode = on
                SessionUi.hint = sendJson(session, """{"op":"set_game_mode","on":$on}""")
            }
            ToggleRow("无音频 30 分钟自动关机", SessionUi.autoPowerOff) { on ->
                SessionUi.autoPowerOff = on
                SessionUi.hint = sendJson(session, """{"op":"set_auto_power_off","on":$on}""")
            }
        }
        Section("播放") {
            FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                listOf(
                    "play" to "播放",
                    "pause" to "暂停",
                    "previous" to "上一曲",
                    "next" to "下一曲",
                    "volume_up" to "音量+",
                    "volume_down" to "音量-",
                ).forEach { (action, label) ->
                    FilledTonalButton(onClick = {
                        SessionUi.hint = sendJson(session, """{"op":"playback","action":"$action"}""")
                    }) { Text(label) }
                }
            }
        }
        Section("危险操作") {
            OutlinedButton(onClick = {
                confirm = ConfirmOp("断开当前主机", "会给耳机发 CD, 当前正在播放的设备会掉线.", """{"op":"disconnect_host"}""")
            }) { Text("断开主机") }
            OutlinedButton(onClick = {
                confirm = ConfirmOp("关机", "耳机会关机.", """{"op":"power_off"}""")
            }) { Text("关机") }
            OutlinedButton(onClick = {
                confirm = ConfirmOp("重新配对", "耳机会进入配对.", """{"op":"re_pair"}""")
            }) { Text("重新配对") }
            Button(
                onClick = {
                    confirm = ConfirmOp("恢复出厂", "会清掉耳机设置.", """{"op":"factory_reset"}""")
                },
                colors = ButtonDefaults.buttonColors(
                    containerColor = MaterialTheme.colorScheme.error,
                    contentColor = MaterialTheme.colorScheme.onError,
                ),
            ) { Text("恢复出厂") }
        }
    }
    confirm?.let { op ->
        AlertDialog(
            onDismissRequest = { confirm = null },
            title = { Text(op.title) },
            text = { Text(op.body) },
            confirmButton = {
                Button(onClick = {
                    SessionUi.hint = sendJson(session, op.json)
                    confirm = null
                }) { Text("发送") }
            },
            dismissButton = {
                TextButton(onClick = { confirm = null }) { Text("取消") }
            },
        )
    }
}

@Composable
private fun Section(title: String, caption: String? = null, content: @Composable ColumnScope.() -> Unit) {
    Card(
        modifier = Modifier.fillMaxWidth(),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceVariant),
    ) {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(title, style = MaterialTheme.typography.titleMedium)
            if (caption != null) {
                Text(caption, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
            content()
        }
    }
}

@Composable
private fun InfoLine(label: String, value: String) {
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
        Text(label, color = MaterialTheme.colorScheme.onSurfaceVariant)
        Text(value)
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun ChipRow(
    options: List<Pair<String, String>>,
    selected: String?,
    onPick: (String) -> Unit,
) {
    FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        options.forEach { (id, label) ->
            FilterChip(selected = selected == id, onClick = { onPick(id) }, label = { Text(label) })
        }
    }
}

@Composable
private fun ToggleRow(label: String, checked: Boolean, onChange: (Boolean) -> Unit) {
    Row(
        Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(label, modifier = Modifier.weight(1f))
        Switch(checked = checked, onCheckedChange = onChange)
    }
}

private fun sendControl(session: Long) {
    val n = listOf(SessionUi.controlNormal, SessionUi.controlReduction, SessionUi.controlAmbient).count { it }
    if (n < 2) {
        SessionUi.hint = "至少选 2 项"
        return
    }
    SessionUi.hint = sendJson(
        session,
        """{"op":"set_control_settings","normal":${SessionUi.controlNormal},"reduction":${SessionUi.controlReduction},"ambient":${SessionUi.controlAmbient}}""",
    )
}
