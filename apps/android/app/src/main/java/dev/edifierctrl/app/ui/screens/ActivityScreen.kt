@file:OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)

package dev.edifierctrl.app.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import dev.edifierctrl.app.session.ActivityEntry
import dev.edifierctrl.app.session.AppActions
import dev.edifierctrl.app.session.SessionState
import dev.edifierctrl.app.ui.components.ConfirmAction
import dev.edifierctrl.app.ui.components.SectionDisclosure
import dev.edifierctrl.app.ui.components.WorkspaceCard
import dev.edifierctrl.app.ui.components.WorkspacePage
import org.json.JSONObject
import java.text.DateFormat
import java.util.Date

private data class DiagnosticCommand(val operation: String, val values: JSONObject)

@Composable
fun ActivityScreen() {
    val scope = "${SessionState.address.orEmpty()}/${SessionState.selectedProfile.id}"
    var payload by rememberSaveable(scope) { mutableStateOf("{\"op\":\"query_battery\"}") }
    var validationError by rememberSaveable(scope) { mutableStateOf<String?>(null) }
    var pending by remember(scope) { mutableStateOf<DiagnosticCommand?>(null) }
    val entries = SessionState.activities.sortedByDescending { it.id }.take(200)
    val timeFormat = remember { DateFormat.getDateTimeInstance(DateFormat.SHORT, DateFormat.MEDIUM) }
    val canInspect = SessionState.nativeAvailable && !SessionState.busy
    val canSend = SessionState.ready && SessionState.canControl && !SessionState.busy

    WorkspacePage("活动", "查看连接, 指令和交接结果, 保留最新 200 条记录.") {
        WorkspaceCard("最近活动", "最新记录在前, 失败详情可长按选择和复制.") {
            OutlinedButton(
                onClick = { AppActions.clearActivities() },
                enabled = entries.isNotEmpty(),
                modifier = Modifier.heightIn(min = 48.dp),
            ) { Text("清空活动") }
            if (entries.isEmpty()) {
                Text("还没有活动记录", style = MaterialTheme.typography.titleSmall)
                Text("连接耳机或使用交接后, 这里会显示结果.", color = MaterialTheme.colorScheme.onSurfaceVariant)
            } else {
                entries.forEachIndexed { index, entry ->
                    key(entry.id) {
                        if (index > 0) HorizontalDivider()
                        ActivityRow(entry, timeFormat)
                    }
                }
            }
        }
        SectionDisclosure("协议诊断", "封装 JSON 命令或解析十六进制帧, 无需连接耳机. 发送命令会实际操作耳机.") {
            OutlinedTextField(
                value = payload,
                onValueChange = {
                    payload = it
                    validationError = null
                },
                label = { Text("JSON 命令或十六进制帧") },
                minLines = 4,
                enabled = !SessionState.busy,
                textStyle = MaterialTheme.typography.bodyMedium.copy(fontFamily = FontFamily.Monospace),
                modifier = Modifier.fillMaxWidth(),
            )
            FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                FilledTonalButton(
                    onClick = {
                        validationError = null
                        AppActions.diagnostic(payload, parse = false)
                    },
                    enabled = canInspect && payload.isNotBlank(),
                    modifier = Modifier.heightIn(min = 48.dp),
                ) { Text("封装 JSON 命令") }
                FilledTonalButton(
                    onClick = {
                        validationError = null
                        AppActions.diagnostic(payload, parse = true)
                    },
                    enabled = canInspect && payload.isNotBlank(),
                    modifier = Modifier.heightIn(min = 48.dp),
                ) { Text("解析十六进制帧") }
            }
            if (!SessionState.nativeAvailable) {
                Text("诊断功能需要可用的本地协议库.", color = MaterialTheme.colorScheme.error)
            }
            if (SessionState.diagnosticOutput.isNotEmpty()) {
                Text("诊断结果", style = MaterialTheme.typography.titleSmall)
                SelectionContainer {
                    Text(SessionState.diagnosticOutput, style = MaterialTheme.typography.bodySmall, fontFamily = FontFamily.Monospace)
                }
            }
            HorizontalDivider()
            Text("发送到当前耳机", style = MaterialTheme.typography.titleSmall)
            Text(
                "仅接受包含字符串 op 的 JSON 对象. 原始命令体使用 raw 与 hex 字段. 发送前会再次确认目标与完整内容, 可能改变设置或中断连接.",
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            validationError?.let { Text(it, color = MaterialTheme.colorScheme.error) }
            FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                OutlinedButton(
                    onClick = {
                        val result = runCatching { parseDiagnosticCommand(payload) }
                        validationError = result.exceptionOrNull()?.message
                        pending = result.getOrNull()
                    },
                    enabled = canSend && payload.isNotBlank(),
                    modifier = Modifier.heightIn(min = 48.dp),
                ) { Text("确认发送 JSON 命令") }
                OutlinedButton(
                    onClick = { AppActions.readout() },
                    enabled = canSend,
                    modifier = Modifier.heightIn(min = 48.dp),
                ) { Text("读取耳机状态") }
            }
            if (!SessionState.connected) {
                Text("发送命令需要先连接耳机. 上方封装与解析仍可使用.", style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        }
    }
    pending?.let { command ->
        val preview = JSONObject(command.values.toString()).put("op", command.operation).toString(2)
        ConfirmAction(
            title = "向当前耳机发送 ${command.operation}",
            detail = "目标: ${SessionState.headphoneName}\n地址: ${SessionState.address.orEmpty()}\n\n$preview\n\n这是实际发送. 关机或恢复出厂等命令可能中断连接或清除配对记录. 原始命令执行后的状态请通过读取耳机状态确认.",
            action = "发送命令",
            onConfirm = {
                if (SessionState.ready && SessionState.canControl && !SessionState.busy) {
                    AppActions.send(command.operation, command.values, "诊断命令: ${command.operation}", diagnosticQuery(command.operation))
                }
                pending = null
            },
            onDismiss = { pending = null },
        )
    }
}

@Composable
private fun ActivityRow(entry: ActivityEntry, timeFormat: DateFormat) {
    Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
        Text(timeFormat.format(Date(entry.time)), style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        Text(
            if (entry.isError) "失败 / ${entry.title}" else entry.title,
            style = MaterialTheme.typography.titleSmall,
            color = if (entry.isError) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurface,
        )
        if (entry.detail.isNotBlank()) {
            SelectionContainer {
                Text(entry.detail, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        }
    }
}

private fun parseDiagnosticCommand(input: String): DiagnosticCommand {
    val json = JSONObject(input)
    val rawOperation = json.opt("op")
    val operation = rawOperation as? String ?: error("请输入包含非空字符串 op 字段的 JSON 对象.")
    require(operation.isNotBlank()) { "请输入包含非空字符串 op 字段的 JSON 对象." }
    val values = JSONObject()
    val keys = json.keys()
    while (keys.hasNext()) {
        val field = keys.next()
        if (field != "op") values.put(field, json.get(field))
    }
    return DiagnosticCommand(operation, values)
}

private fun diagnosticQuery(operation: String): String? = when (operation) {
    "set_noise_mode", "set_ambient_volume" -> "query_noise"
    "set_sound_effect" -> "query_sound_effect"
    "set_control_settings" -> "query_control_settings"
    "set_game_mode" -> "query_game_mode"
    "set_prompt_volume" -> "query_prompt_volume"
    "set_shutdown_timer", "disable_shutdown_timer" -> "query_shutdown_timer"
    "set_name" -> "query_name"
    "set_auto_power_off" -> "query_auto_power_off"
    "playback" -> "query_playback"
    else -> null
}
