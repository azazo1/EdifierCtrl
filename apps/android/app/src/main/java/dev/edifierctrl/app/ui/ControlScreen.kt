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
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.Check
import androidx.compose.material.icons.automirrored.outlined.Undo
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.FilledIconButton
import androidx.compose.material3.FilledTonalIconButton
import androidx.compose.material3.LocalMinimumInteractiveComponentSize
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
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
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import dev.edifierctrl.app.EdifierNative
import kotlin.math.roundToInt
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

private data class ConfirmOp(val title: String, val body: String, val json: String)

private class CardDraft<T>(committed: T) {
    var value by mutableStateOf(committed)
        private set
    var dirty by mutableStateOf(false)
        private set

    fun edit(next: T, committed: T) {
        value = next
        dirty = next != committed
    }

    fun revert(committed: T) {
        value = committed
        dirty = false
    }

    fun markApplied() {
        dirty = false
    }

    fun syncFrom(committed: T) {
        if (!dirty) {
            value = committed
        }
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
fun ControlScreen() {
    var confirm by remember { mutableStateOf<ConfirmOp?>(null) }
    val session = remember { EdifierNative.ensureSession() }
    val scope = rememberCoroutineScope()
    val name = rememberDraft(SessionUi.deviceName.orEmpty())
    val noise = rememberDraft(SessionUi.noise)
    val ambient = rememberDraft(SessionUi.ambientVolume)
    val effect = rememberDraft(SessionUi.effect)
    val prompt = rememberDraft(SessionUi.promptVolume)
    val shutdownOn = rememberDraft(SessionUi.shutdownOn)
    val shutdownMin = rememberDraft(SessionUi.shutdownMinutes)
    val ldac = rememberDraft(SessionUi.ldac)
    val game = rememberDraft(SessionUi.gameMode)
    val autoOff = rememberDraft(SessionUi.autoPowerOff)
    val csNormal = rememberDraft(SessionUi.controlNormal)
    val csReduction = rememberDraft(SessionUi.controlReduction)
    val csAmbient = rememberDraft(SessionUi.controlAmbient)
    val shutdownDirty = shutdownOn.dirty || shutdownMin.dirty
    val switchDirty = game.dirty || autoOff.dirty
    val controlDirty = csNormal.dirty || csReduction.dirty || csAmbient.dirty
    Column(
        Modifier
            .fillMaxSize()
            .padding(horizontal = 20.dp, vertical = 12.dp)
            .verticalScroll(rememberScrollState()),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Text("控制", style = MaterialTheme.typography.headlineSmall)
        Text(
            if (SessionUi.connected) "改完点卡片右上角对号才会发到耳机." else "先到设备页连接耳机.",
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
        Section(
            "名称",
            "改完要删配对记录再配对才生效.",
            dirty = name.dirty,
            onRevert = { name.revert(SessionUi.deviceName.orEmpty()) },
            onApply = {
                val escaped = name.value.replace("\\", "\\\\").replace("\"", "\\\"")
                SessionUi.hint = sendJson(session, """{"op":"set_name","name":"$escaped"}""")
                SessionUi.deviceName = name.value
                name.markApplied()
            },
        ) {
            OutlinedTextField(
                value = name.value,
                onValueChange = { name.edit(it, SessionUi.deviceName.orEmpty()) },
                label = { Text("耳机名") },
                modifier = Modifier.fillMaxWidth(),
                singleLine = true,
            )
        }
        Section(
            "降噪",
            dirty = noise.dirty,
            onRevert = { noise.revert(SessionUi.noise) },
            onApply = {
                val mode = noise.value
                if (mode != null) {
                    SessionUi.hint = sendJson(session, """{"op":"set_noise_mode","mode":"$mode"}""")
                    SessionUi.noise = mode
                    noise.markApplied()
                }
            },
        ) {
            ChipRow(
                options = listOf("normal" to "关闭", "reduction" to "降噪", "ambient" to "通透"),
                selected = noise.value,
            ) { mode -> noise.edit(mode, SessionUi.noise) }
        }
        Section(
            "通透音量",
            "仅通透模式有效, -3 到 3.",
            dirty = ambient.dirty,
            onRevert = { ambient.revert(SessionUi.ambientVolume) },
            onApply = {
                SessionUi.hint = sendJson(
                    session,
                    """{"op":"set_ambient_volume","volume":${ambient.value}}""",
                )
                SessionUi.ambientVolume = ambient.value
                ambient.markApplied()
            },
        ) {
            Text("${ambient.value}")
            Slider(
                value = ambient.value.toFloat(),
                onValueChange = { ambient.edit(it.roundToInt(), SessionUi.ambientVolume) },
                valueRange = -3f..3f,
                steps = 5,
            )
        }
        Section(
            "按键可切换的模式",
            "至少选 2 项.",
            dirty = controlDirty,
            onRevert = {
                csNormal.revert(SessionUi.controlNormal)
                csReduction.revert(SessionUi.controlReduction)
                csAmbient.revert(SessionUi.controlAmbient)
            },
            onApply = {
                val n = listOf(csNormal.value, csReduction.value, csAmbient.value).count { it }
                if (n < 2) {
                    SessionUi.hint = "至少选 2 项"
                } else {
                    SessionUi.hint = sendJson(
                        session,
                        """{"op":"set_control_settings","normal":${csNormal.value},"reduction":${csReduction.value},"ambient":${csAmbient.value}}""",
                    )
                    SessionUi.controlNormal = csNormal.value
                    SessionUi.controlReduction = csReduction.value
                    SessionUi.controlAmbient = csAmbient.value
                    csNormal.markApplied()
                    csReduction.markApplied()
                    csAmbient.markApplied()
                }
            },
        ) {
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                FilterChip(
                    selected = csNormal.value,
                    onClick = { csNormal.edit(!csNormal.value, SessionUi.controlNormal) },
                    label = { Text("关闭") },
                )
                FilterChip(
                    selected = csReduction.value,
                    onClick = { csReduction.edit(!csReduction.value, SessionUi.controlReduction) },
                    label = { Text("降噪") },
                )
                FilterChip(
                    selected = csAmbient.value,
                    onClick = { csAmbient.edit(!csAmbient.value, SessionUi.controlAmbient) },
                    label = { Text("通透") },
                )
            }
        }
        Section(
            "音效",
            dirty = effect.dirty,
            onRevert = { effect.revert(SessionUi.effect) },
            onApply = {
                val v = effect.value
                if (v != null) {
                    SessionUi.hint = sendJson(session, """{"op":"set_sound_effect","effect":"$v"}""")
                    SessionUi.effect = v
                    effect.markApplied()
                }
            },
        ) {
            ChipRow(
                options = listOf(
                    "normal" to "标准",
                    "pop" to "流行",
                    "classical" to "古典",
                    "rock" to "摇滚",
                ),
                selected = effect.value,
            ) { v -> effect.edit(v, SessionUi.effect) }
        }
        Section(
            "提示音量",
            dirty = prompt.dirty,
            onRevert = { prompt.revert(SessionUi.promptVolume) },
            onApply = {
                SessionUi.hint = sendJson(
                    session,
                    """{"op":"set_prompt_volume","volume":${prompt.value}}""",
                )
                SessionUi.promptVolume = prompt.value
                prompt.markApplied()
            },
        ) {
            Text("${prompt.value}")
            Slider(
                value = prompt.value.toFloat(),
                onValueChange = { prompt.edit(it.roundToInt(), SessionUi.promptVolume) },
                valueRange = 0f..15f,
                steps = 14,
            )
        }
        Section(
            "定时关机",
            "断电后会回到默认.",
            dirty = shutdownDirty,
            onRevert = {
                shutdownOn.revert(SessionUi.shutdownOn)
                shutdownMin.revert(SessionUi.shutdownMinutes)
            },
            onApply = {
                SessionUi.hint = if (shutdownOn.value) {
                    sendJson(session, """{"op":"set_shutdown_timer","minutes":${shutdownMin.value}}""")
                } else {
                    sendJson(session, """{"op":"disable_shutdown_timer"}""")
                }
                SessionUi.shutdownOn = shutdownOn.value
                SessionUi.shutdownMinutes = shutdownMin.value
                shutdownOn.markApplied()
                shutdownMin.markApplied()
            },
        ) {
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.SpaceBetween,
                modifier = Modifier.fillMaxWidth(),
            ) {
                Text(if (shutdownOn.value) "已开启" else "关闭")
                Switch(
                    checked = shutdownOn.value,
                    onCheckedChange = { shutdownOn.edit(it, SessionUi.shutdownOn) },
                )
            }
            Text("${shutdownMin.value} 分钟")
            Slider(
                value = shutdownMin.value.toFloat(),
                onValueChange = {
                    shutdownMin.edit(it.roundToInt().coerceIn(1, 180), SessionUi.shutdownMinutes)
                },
                valueRange = 1f..180f,
                steps = 178,
                enabled = shutdownOn.value,
            )
        }
        Section(
            "LDAC",
            "改完要重新配对才生效, 同时会关掉定时关机.",
            dirty = ldac.dirty,
            onRevert = { ldac.revert(SessionUi.ldac) },
            onApply = {
                val mode = ldac.value
                if (mode != null) {
                    SessionUi.hint = sendJson(session, """{"op":"set_ldac","mode":"$mode"}""")
                    SessionUi.ldac = mode
                    ldac.markApplied()
                }
            },
        ) {
            ChipRow(
                options = listOf("off" to "关闭", "rate48k" to "44.1k / 48k", "rate96k" to "96k"),
                selected = ldac.value,
            ) { mode -> ldac.edit(mode, SessionUi.ldac) }
        }
        Section(
            "开关",
            dirty = switchDirty,
            onRevert = {
                game.revert(SessionUi.gameMode)
                autoOff.revert(SessionUi.autoPowerOff)
            },
            onApply = {
                if (game.dirty) {
                    SessionUi.hint = sendJson(session, """{"op":"set_game_mode","on":${game.value}}""")
                    SessionUi.gameMode = game.value
                    game.markApplied()
                }
                if (autoOff.dirty) {
                    SessionUi.hint = sendJson(session, """{"op":"set_auto_power_off","on":${autoOff.value}}""")
                    SessionUi.autoPowerOff = autoOff.value
                    autoOff.markApplied()
                }
            },
        ) {
            ToggleRow("游戏模式", game.value) { game.edit(it, SessionUi.gameMode) }
            ToggleRow("无音频 30 分钟自动关机", autoOff.value) { autoOff.edit(it, SessionUi.autoPowerOff) }
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
private fun <T> rememberDraft(committed: T): CardDraft<T> {
    val draft = remember { CardDraft(committed) }
    LaunchedEffect(committed) {
        draft.syncFrom(committed)
    }
    return draft
}

@Composable
private fun Section(
    title: String,
    caption: String? = null,
    dirty: Boolean = false,
    onRevert: (() -> Unit)? = null,
    onApply: (() -> Unit)? = null,
    content: @Composable ColumnScope.() -> Unit,
) {
    Card(
        modifier = Modifier.fillMaxWidth(),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceVariant),
    ) {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Row(
                Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(6.dp),
            ) {
                Text(title, style = MaterialTheme.typography.titleMedium, modifier = Modifier.weight(1f))
                if (dirty && onRevert != null && onApply != null) {
                    CompositionLocalProvider(LocalMinimumInteractiveComponentSize provides Dp.Unspecified) {
                        FilledTonalIconButton(onClick = onRevert, modifier = Modifier.size(28.dp)) {
                            Icon(Icons.AutoMirrored.Outlined.Undo, contentDescription = "撤回", modifier = Modifier.size(14.dp))
                        }
                        FilledIconButton(onClick = onApply, modifier = Modifier.size(28.dp)) {
                            Icon(Icons.Outlined.Check, contentDescription = "生效", modifier = Modifier.size(14.dp))
                        }
                    }
                }
            }
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
