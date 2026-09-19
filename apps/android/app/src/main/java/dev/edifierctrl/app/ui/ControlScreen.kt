@file:OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)

package dev.edifierctrl.app.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.selection.triStateToggleable
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.Headphones
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TriStateCheckbox
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveableStateHolder
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.state.ToggleableState
import androidx.compose.ui.unit.dp
import dev.edifierctrl.app.infrastructure.AppPreferences
import dev.edifierctrl.app.session.AppActions
import dev.edifierctrl.app.session.SessionState
import dev.edifierctrl.app.session.effectLabel
import dev.edifierctrl.app.session.ldacLabel
import dev.edifierctrl.app.session.noiseLabel
import dev.edifierctrl.app.session.normalizeAddress
import dev.edifierctrl.app.session.sameAddress
import dev.edifierctrl.app.ui.components.ConfirmAction
import dev.edifierctrl.app.ui.components.ConnectionSummary
import dev.edifierctrl.app.ui.components.DraftActions
import dev.edifierctrl.app.ui.components.EmptyMessage
import dev.edifierctrl.app.ui.components.NumberDraftField
import dev.edifierctrl.app.ui.components.SectionDisclosure
import dev.edifierctrl.app.ui.components.WorkspaceCard
import dev.edifierctrl.app.ui.components.WorkspacePage
import dev.edifierctrl.app.ui.components.rememberReadingDraft
import org.json.JSONObject

private data class ControlConfirmation(
    val title: String,
    val detail: String,
    val operation: String,
    val values: JSONObject = JSONObject(),
    val query: String? = null,
)

@Composable
fun ControlScreen(onDevices: () -> Unit = {}, onHandoff: () -> Unit = {}) {
    val scope = "${normalizeAddress(SessionState.address) ?: "disconnected"}/${SessionState.selectedProfile.id}"
    val enabled = SessionState.ready && SessionState.canControl && !SessionState.busy
    val drafts = rememberSaveableStateHolder()
    var confirmation by remember(scope) { mutableStateOf<ControlConfirmation?>(null) }

    WorkspacePage("耳机", "控制音效, 管理连接, 在设备之间切换.") {
        HeadphoneHero(onDevices, onHandoff)
        if (SessionState.connected) {
            Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                ConnectionSummary()
                Text(if (SessionState.groupJoined) "交接组内 ${SessionState.peers.size + 1} 台设备" else "尚未加入交接组")
                OutlinedButton(onClick = onHandoff, modifier = Modifier.heightIn(min = 48.dp)) { Text("前往交接") }
            }
            if (SessionState.supports("noise")) {
                WorkspaceCard("聆听模式", "隔绝喧闹, 或留意身边的声音.") {
                    ConfirmedChoices(
                        reading = noiseLabel(SessionState.noise),
                        options = buildList {
                            add("normal" to "标准")
                            add("reduction" to "主动降噪")
                            if (SessionState.supports("ambient_sound")) add("ambient" to "环境声")
                        },
                        selected = SessionState.noise,
                        enabled = enabled,
                    ) {
                        AppActions.send("set_noise_mode", JSONObject().put("mode", it), "切换聆听模式", "query_noise")
                    }
                    if (SessionState.supports("ambient_sound") && SessionState.noise == "ambient") {
                        HorizontalDivider()
                        drafts.SaveableStateProvider("ambient/$scope") {
                            NumberDraftField(
                                title = "环境声强度",
                                confirmed = SessionState.ambientVolume,
                                range = -3..3,
                                scope = scope,
                                field = "ambient",
                                enabled = enabled,
                                detail = "仅在环境声模式下生效.",
                            ) {
                                AppActions.send("set_ambient_volume", JSONObject().put("volume", it), "设置环境声强度", "query_noise")
                            }
                        }
                    }
                }
            }
            if (SessionState.supports("sound_effect") || SessionState.supports("playback")) {
                WorkspaceCard("声音与播放", "按钮高亮来自耳机回报. 播放指令发出后等待读取确认.") {
                    if (SessionState.supports("sound_effect")) {
                        Text("声音风格", style = MaterialTheme.typography.titleSmall)
                        ConfirmedChoices(
                            reading = effectLabel(SessionState.effect),
                            options = listOf("normal" to "标准", "pop" to "流行", "classical" to "古典", "rock" to "摇滚"),
                            selected = SessionState.effect,
                            enabled = enabled,
                        ) {
                            AppActions.send("set_sound_effect", JSONObject().put("effect", it), "切换声音风格", "query_sound_effect")
                        }
                    }
                    if (SessionState.supports("playback")) {
                        if (SessionState.supports("sound_effect")) HorizontalDivider()
                        Text("播放控制", style = MaterialTheme.typography.titleSmall)
                        Text(SessionState.audioLabel, color = MaterialTheme.colorScheme.onSurfaceVariant)
                        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                            listOf(
                                "previous" to "上一曲", "play" to "播放", "pause" to "暂停",
                                "next" to "下一曲", "volume_down" to "降低音量", "volume_up" to "提高音量",
                            ).forEach { (action, label) ->
                                FilledTonalButton(
                                    onClick = {
                                        AppActions.send("playback", JSONObject().put("action", action), label, "query_playback")
                                    },
                                    enabled = enabled,
                                    modifier = Modifier.heightIn(min = 48.dp),
                                ) { Text(label) }
                            }
                        }
                    }
                }
            }
            if (listOf("game_mode", "auto_power_off", "prompt_volume", "shutdown_timer", "control_settings", "ldac", "name").any(SessionState::supports)) {
                SectionDisclosure("高级设置", "草稿只在点击应用后发送, 状态仍以耳机回报为准.") {
                    drafts.SaveableStateProvider("advanced/$scope") {
                        AdvancedSettings(scope, enabled) { confirmation = it }
                    }
                }
            }
        }
        SectionDisclosure("设备信息与机型", "按机型显示功能, 通用档案中的部分能力可能不被耳机支持.") {
            SelectionContainer {
                Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text("蓝牙地址: ${SessionState.mac ?: SessionState.address ?: "等待连接"}")
                    Text("固件版本: ${SessionState.firmware ?: "等待耳机回报"}")
                }
            }
            Text("机型档案", style = MaterialTheme.typography.titleSmall)
            ConfirmedChoices(
                reading = SessionState.selectedProfile.displayName,
                options = SessionState.profiles.map { it.id to it.displayName },
                selected = SessionState.selectedProfile.id,
                enabled = SessionState.ready && !SessionState.busy,
            ) { AppActions.selectProfile(it) }
        }
        if (SessionState.connected) {
            SectionDisclosure("设备维护", "这些操作可能中断连接. 恢复出厂还会清除设置和配对记录.") {
                FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                    listOf(
                        "disconnect_host" to "释放当前主机", "power_off" to "关闭耳机",
                        "re_pair" to "重新配对", "factory_reset" to "恢复出厂",
                    ).forEach { (operation, label) ->
                        OutlinedButton(
                            onClick = { confirmation = maintenanceConfirmation(operation, label) },
                            enabled = enabled,
                            modifier = Modifier.heightIn(min = 48.dp),
                        ) { Text(label, color = MaterialTheme.colorScheme.error) }
                    }
                }
            }
        }
    }
    confirmation?.let { pending ->
        ConfirmAction(
            title = pending.title,
            detail = pending.detail,
            action = pending.title,
            onConfirm = {
                if (SessionState.ready && SessionState.canControl && !SessionState.busy) {
                    AppActions.send(pending.operation, pending.values, pending.title, pending.query)
                }
                confirmation = null
            },
            onDismiss = { confirmation = null },
        )
    }
}

@Composable
private fun HeadphoneHero(onDevices: () -> Unit, onHandoff: () -> Unit) {
    val ready = SessionState.ready && !SessionState.busy
    Card(colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.primaryContainer), modifier = Modifier.fillMaxWidth()) {
        Column(Modifier.padding(20.dp), verticalArrangement = Arrangement.spacedBy(16.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                Box(
                    modifier = Modifier.size(76.dp).background(MaterialTheme.colorScheme.primary.copy(alpha = 0.08f), CircleShape),
                    contentAlignment = Alignment.Center,
                ) {
                    Icon(Icons.Outlined.Headphones, contentDescription = null, modifier = Modifier.size(48.dp), tint = MaterialTheme.colorScheme.primary)
                }
                Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                    Text("EDIFIER / PERSONAL AUDIO", style = MaterialTheme.typography.labelSmall)
                    Text(SessionState.headphoneName, style = MaterialTheme.typography.headlineSmall)
                    Text(if (SessionState.connected) "控制已连接" else "控制未连接", style = MaterialTheme.typography.labelLarge)
                    Text(SessionState.audioLabel, style = MaterialTheme.typography.bodySmall)
                }
            }
            val battery = SessionState.battery
            Text(battery?.let { "电量 $it%" } ?: "电量: 等待耳机回报")
            if (battery != null) {
                LinearProgressIndicator(progress = { battery.coerceIn(0, 100) / 100f }, modifier = Modifier.fillMaxWidth())
            }
            if (!SessionState.connected) {
                EmptyMessage("准备好, 开始聆听", "先在 Android 蓝牙设置中完成配对, 再选择耳机.")
            }
            FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                if (SessionState.connected) {
                    FilledTonalButton(
                        onClick = { AppActions.readout() }, enabled = ready && SessionState.canControl,
                        modifier = Modifier.heightIn(min = 48.dp),
                    ) { Text("读取状态") }
                    OutlinedButton(
                        onClick = { AppActions.disconnect() }, enabled = ready,
                        modifier = Modifier.heightIn(min = 48.dp),
                    ) { Text("断开控制") }
                } else {
                    FilledTonalButton(onClick = onDevices, modifier = Modifier.heightIn(min = 48.dp)) { Text("选择耳机") }
                    val lastAddress = normalizeAddress(AppPreferences.lastDeviceAddress)
                    if (lastAddress != null) {
                        val holder = SessionState.peers.firstOrNull {
                            sameAddress(it.holding, lastAddress) && !sameAddress(SessionState.holding, lastAddress)
                        }
                        OutlinedButton(
                            onClick = {
                                if (holder != null) {
                                    AppActions.claimPeer(holder.id)
                                    onHandoff()
                                } else {
                                    AppActions.connect(lastAddress, AppPreferences.lastDeviceName)
                                }
                            },
                            enabled = ready && (holder == null || (holder.canAudio && SessionState.groupJoined)),
                            modifier = Modifier.heightIn(min = 48.dp),
                        ) { Text(if (holder == null) "连接上次耳机" else "接管上次耳机") }
                    }
                }
            }
        }
    }
}

@Composable
private fun AdvancedSettings(scope: String, enabled: Boolean, onConfirm: (ControlConfirmation) -> Unit) {
    if (SessionState.supports("game_mode")) {
        BooleanSetting("游戏模式", SessionState.gameMode, enabled) {
            AppActions.send("set_game_mode", JSONObject().put("on", it), "设置游戏模式", "query_game_mode")
        }
    }
    if (SessionState.supports("auto_power_off")) {
        BooleanSetting("无音频时自动关机", SessionState.autoPowerOff, enabled) {
            AppActions.send("set_auto_power_off", JSONObject().put("on", it), "设置自动关机", "query_auto_power_off")
        }
    }
    if (SessionState.supports("prompt_volume")) {
        NumberDraftField("提示音量", SessionState.promptVolume, 0..15, scope, "prompt", enabled) {
            AppActions.send("set_prompt_volume", JSONObject().put("volume", it), "设置提示音量", "query_prompt_volume")
        }
    }
    if (SessionState.supports("shutdown_timer")) {
        HorizontalDivider()
        Text(
            when (SessionState.shutdownOn) {
                true -> SessionState.shutdownMinutes?.let { "定时关机已开启, 耳机回报 $it 分钟" } ?: "定时关机已开启, 等待分钟数"
                false -> "定时关机未开启"
                null -> "定时关机: 等待耳机回报"
            },
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        NumberDraftField(
            "定时关机", SessionState.shutdownMinutes, 1..180, scope, "timer", enabled,
            unit = "分钟", allowUnchanged = SessionState.shutdownOn != true,
        ) {
            AppActions.send("set_shutdown_timer", JSONObject().put("minutes", it), "设置定时关机", "query_shutdown_timer")
        }
        OutlinedButton(
            onClick = { AppActions.send("disable_shutdown_timer", label = "关闭定时关机", query = "query_shutdown_timer") },
            enabled = enabled && SessionState.shutdownOn == true,
            modifier = Modifier.heightIn(min = 48.dp),
        ) { Text("关闭定时关机") }
    }
    if (SessionState.supports("control_settings")) {
        HorizontalDivider()
        ControlModesDraft(scope, enabled)
    }
    if (SessionState.supports("name")) {
        HorizontalDivider()
        NameDraft(scope, enabled)
    }
    if (SessionState.supports("ldac")) {
        HorizontalDivider()
        Text("LDAC", style = MaterialTheme.typography.titleSmall)
        ConfirmedChoices(
            reading = ldacLabel(SessionState.ldac),
            options = listOf("off" to "关闭", "rate48k" to "44.1k / 48k", "rate96k" to "96k"),
            selected = SessionState.ldac,
            enabled = enabled,
        ) { mode ->
            onConfirm(
                ControlConfirmation(
                    "更改 LDAC 设置",
                    "将设为 ${ldacLabel(mode)}. 耳机可能中断蓝牙连接, 并关闭定时关机. 系统是否实际使用 LDAC 取决于设备与 Android 音频设置.",
                    "set_ldac",
                    JSONObject().put("mode", mode),
                ),
            )
        }
    }
}

@Composable
private fun NameDraft(scope: String, enabled: Boolean) {
    val confirmed = SessionState.deviceName.orEmpty()
    val draft = rememberReadingDraft(scope, "name", confirmed)
    val byteCount = draft.text.toByteArray(Charsets.UTF_8).size
    val maxBytes = SessionState.selectedProfile.maxNameLen
    val valid = draft.text.isNotBlank() && byteCount <= maxBytes
    Text("耳机名称", style = MaterialTheme.typography.titleSmall)
    Text("当前: ${confirmed.ifEmpty { "等待耳机回报" }}", color = MaterialTheme.colorScheme.onSurfaceVariant)
    OutlinedTextField(
        value = draft.text,
        onValueChange = draft::edit,
        enabled = enabled,
        label = { Text("待应用名称") },
        supportingText = { Text("$byteCount / $maxBytes UTF-8 字节. 更名后可能需要重新配对.") },
        isError = byteCount > maxBytes,
        singleLine = true,
        modifier = Modifier.fillMaxWidth().onFocusChanged { draft.focused = it.isFocused },
    )
    DraftActions(
        changed = draft.text != confirmed,
        enabled = enabled,
        valid = valid,
        onReset = { draft.reset(confirmed) },
        onApply = { AppActions.send("set_name", JSONObject().put("name", draft.text), "更新耳机名称", "query_name") },
    )
}

@Composable
private fun ControlModesDraft(scope: String, enabled: Boolean) {
    val ambientSupported = SessionState.supports("ambient_sound")
    val values = listOf(SessionState.controlNormal, SessionState.controlReduction) +
        if (ambientSupported) listOf(SessionState.controlAmbient) else emptyList()
    val reading = values.joinToString("") { when (it) { true -> "1"; false -> "0"; null -> "?" } }
    val draft = rememberReadingDraft(scope, "button-modes", reading)
    val labels = listOf("标准", "降噪") + if (ambientSupported) listOf("通透") else emptyList()
    val valid = draft.text.length == labels.size && '?' !in draft.text && draft.text.count { it == '1' } >= 2
    Text("按键可切换的模式", style = MaterialTheme.typography.titleSmall)
    Text(
        if (values.any { it == null }) "部分状态尚未回报. 请明确每项草稿并至少选择 2 项." else "当前: ${labels.filterIndexed { index, _ -> values[index] == true }.joinToString(" / ")}. 至少选择 2 项.",
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
    FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
        labels.forEachIndexed { index, label ->
            val value = draft.text.getOrNull(index) ?: '?'
            val state = when (value) { '1' -> ToggleableState.On; '0' -> ToggleableState.Off; else -> ToggleableState.Indeterminate }
            Row(
                modifier = Modifier.heightIn(min = 48.dp).triStateToggleable(
                    state = state,
                    enabled = enabled,
                    role = Role.Checkbox,
                    onClick = {
                        val next = draft.text.padEnd(labels.size, '?').toCharArray()
                        next[index] = if (value == '1') '0' else '1'
                        draft.edit(String(next))
                    },
                ).padding(end = 12.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                TriStateCheckbox(state = state, onClick = null, enabled = enabled)
                Text(if (value == '?') "$label (未读取)" else label)
            }
        }
    }
    DraftActions(
        changed = draft.text != reading,
        enabled = enabled,
        valid = valid,
        onReset = { draft.reset(reading) },
        onApply = {
            AppActions.send(
                "set_control_settings",
                JSONObject().put("normal", draft.text[0] == '1').put("reduction", draft.text[1] == '1')
                    .put("ambient", ambientSupported && draft.text.getOrNull(2) == '1'),
                "设置按键模式",
                "query_control_settings",
            )
        },
    )
}

@Composable
private fun BooleanSetting(title: String, confirmed: Boolean?, enabled: Boolean, onChange: (Boolean) -> Unit) {
    Text(title, style = MaterialTheme.typography.titleSmall)
    ConfirmedChoices(
        reading = when (confirmed) { true -> "开启"; false -> "关闭"; null -> "等待耳机回报" },
        options = listOf("on" to "开启", "off" to "关闭"),
        selected = when (confirmed) { true -> "on"; false -> "off"; null -> null },
        enabled = enabled,
    ) { onChange(it == "on") }
}

@Composable
private fun ConfirmedChoices(
    reading: String,
    options: List<Pair<String, String>>,
    selected: String?,
    enabled: Boolean,
    onPick: (String) -> Unit,
) {
    Text("当前: $reading", color = MaterialTheme.colorScheme.onSurfaceVariant)
    FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
        options.forEach { (id, label) ->
            FilterChip(
                selected = selected == id,
                onClick = { if (id != selected) onPick(id) },
                enabled = enabled,
                label = { Text(label) },
                modifier = Modifier.heightIn(min = 48.dp),
            )
        }
    }
}

private fun maintenanceConfirmation(operation: String, title: String) = ControlConfirmation(
    title = title,
    detail = when (operation) {
        "disconnect_host" -> "耳机将断开当前主机的蓝牙连接. 日常切换设备请优先使用交接功能."
        "power_off" -> "耳机将关机, 当前播放会立即停止."
        "re_pair" -> "耳机将进入配对模式并中断当前连接."
        else -> "这会清除耳机设置和配对记录, 之后需要在各台设备上重新配对."
    },
    operation = operation,
)
