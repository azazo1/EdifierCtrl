@file:OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)

package dev.edifierctrl.app.ui

import android.os.Build
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.unit.dp
import dev.edifierctrl.app.infrastructure.AppPreferences
import dev.edifierctrl.app.session.AppActions
import dev.edifierctrl.app.session.GroupPeer
import dev.edifierctrl.app.session.HandoffProgress
import dev.edifierctrl.app.session.SessionState
import dev.edifierctrl.app.session.normalizeAddress
import dev.edifierctrl.app.session.sameAddress
import dev.edifierctrl.app.ui.components.ConfirmAction
import dev.edifierctrl.app.ui.components.EmptyMessage
import dev.edifierctrl.app.ui.components.SectionDisclosure
import dev.edifierctrl.app.ui.components.WorkspaceCard
import dev.edifierctrl.app.ui.components.WorkspacePage
import dev.edifierctrl.app.ui.components.rememberReadingDraft

@Composable
fun GroupScreen(onBluetoothSettings: () -> Unit = {}) {
    val group = rememberReadingDraft("handoff", "group-name", AppPreferences.groupName)
    var manualAddress by rememberSaveable { mutableStateOf("") }
    var confirmLeave by rememberSaveable { mutableStateOf(false) }
    val normalizedManual = normalizeAddress(manualAddress)
    val enabled = SessionState.ready && !SessionState.busy
    val clipboard = LocalClipboardManager.current

    WorkspacePage("交接", "让 Android, macOS, Windows 和 Linux 在同一局域网中共享耳机.") {
        WorkspaceCard(if (SessionState.groupJoined) "设备已在同一交接组" else "把你的设备连在一起") {
            if (SessionState.groupJoined) {
                Text("当前组名", style = MaterialTheme.typography.labelLarge)
                SelectionContainer {
                    Text(SessionState.joinedGroupName, style = MaterialTheme.typography.titleLarge)
                }
                Text("当前加入的组与已保存设置独立. 成员会自动发现和更新.", color = MaterialTheme.colorScheme.onSurfaceVariant)
                FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                    OutlinedButton(
                        onClick = { clipboard.setText(AnnotatedString(SessionState.joinedGroupName)) },
                        modifier = Modifier.heightIn(min = 48.dp),
                    ) { Text("复制组名") }
                    OutlinedButton(
                        onClick = {
                            if (SessionState.handoff?.active == true) confirmLeave = true else AppActions.leave()
                        },
                        enabled = SessionState.ready && SessionState.operation == null,
                        modifier = Modifier.heightIn(min = 48.dp),
                    ) { Text("退出交接组") }
                }
            } else {
                OutlinedTextField(
                    value = group.text,
                    onValueChange = group::edit,
                    label = { Text("组名") },
                    supportingText = { Text("输入与其他设备完全相同的组名. 首尾空格也会保留.") },
                    singleLine = true,
                    enabled = enabled,
                    modifier = Modifier.fillMaxWidth().onFocusChanged { group.focused = it.isFocused },
                )
                PreferenceToggle("记住组名", AppPreferences.rememberGroup, !SessionState.busy) {
                    group.edit(group.text)
                    AppPreferences.setRememberGroup(it)
                }
                PreferenceToggle(
                    "启动时自动加入",
                    AppPreferences.autoJoinGroup,
                    AppPreferences.rememberGroup && !SessionState.busy,
                ) { AppPreferences.setAutoJoinGroup(it) }
                if (!AppPreferences.rememberGroup) {
                    Text("自动加入需要先记住组名.", style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
                FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                    FilledTonalButton(
                        onClick = { AppActions.join(group.text, AppPreferences.rememberGroup) },
                        enabled = enabled && group.text.isNotBlank(),
                        modifier = Modifier.heightIn(min = 48.dp),
                    ) { Text("加入交接组") }
                    OutlinedButton(
                        onClick = { group.reset(AppPreferences.groupName) },
                        enabled = AppPreferences.groupName.isNotEmpty() && !SessionState.busy,
                        modifier = Modifier.heightIn(min = 48.dp),
                    ) { Text("读取已保存组名") }
                }
            }
            AppPreferences.saveError?.let {
                Text("组设置保存失败: $it", color = MaterialTheme.colorScheme.error)
            }
        }
        SessionState.handoff?.let { HandoffCard(it, onBluetoothSettings) }
        if (SessionState.groupJoined) {
            WorkspaceCard("${Build.MODEL} / 本机", "Android") {
                SelectionContainer {
                    Text(SessionState.holding?.let { "正在持有 $it" } ?: "尚未确认持有耳机音频")
                }
                Text(SessionState.audioLabel, color = MaterialTheme.colorScheme.onSurfaceVariant)
                Text(if (SessionState.connected) "控制通道已连接" else "控制通道未连接", color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
            Text("附近的成员 (${SessionState.peers.size})", style = MaterialTheme.typography.titleMedium)
            if (SessionState.peers.isEmpty()) {
                EmptyMessage("等待你的其他设备", "在其他客户端加入相同组名. 确保处于同一 Wi-Fi 或局域网, 允许本地网络访问, 并关闭路由器的设备隔离.")
            }
            SessionState.peers.forEach { peer ->
                key(peer.id) { PeerCard(peer, enabled) }
            }
            SectionDisclosure("通过耳机地址接管", "耳机须已与这台 Android 配对, 原设备客户端须加入同一交接组.") {
                OutlinedTextField(
                    value = manualAddress,
                    onValueChange = { manualAddress = it },
                    label = { Text("耳机 MAC 地址") },
                    placeholder = { Text("AA:BB:CC:DD:EE:FF") },
                    isError = manualAddress.isNotBlank() && normalizedManual == null,
                    supportingText = { Text("请输入 12 位十六进制地址, 可带冒号或短横线.") },
                    singleLine = true,
                    enabled = enabled,
                    keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Characters),
                    modifier = Modifier.fillMaxWidth(),
                )
                FilledTonalButton(
                    onClick = { normalizedManual?.let { AppActions.claim(it) } },
                    enabled = enabled && normalizedManual != null && !sameAddress(SessionState.holding, normalizedManual),
                    modifier = Modifier.heightIn(min = 48.dp),
                ) { Text("接管到这台 Android") }
            }
        }
        WorkspaceCard("交接前, 准备这三件事") {
            Text("1. 在每台手机和电脑上分别完成一次耳机蓝牙配对.")
            Text("2. 各端保持 EdifierCtrl 运行, 使用同一局域网和完全相同的组名.")
            Text("3. 在目标设备点击接管. 系统确认音频已连接后才算完成.")
            Text(
                "部分 Android 系统限制应用自动连接音频. 如未接通, 请到系统蓝牙设置连接耳机, 再查看交接结果.",
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            OutlinedButton(onClick = onBluetoothSettings, modifier = Modifier.heightIn(min = 48.dp)) { Text("打开蓝牙设置") }
        }
    }
    if (confirmLeave && SessionState.groupJoined) {
        ConfirmAction(
            title = "退出并停止当前交接",
            detail = "将退出 ${SessionState.joinedGroupName}, 当前交接也会停止. 系统音频连接请在退出后检查.",
            action = "退出交接组",
            onConfirm = {
                if (SessionState.ready && SessionState.operation == null) AppActions.leave()
                confirmLeave = false
            },
            onDismiss = { confirmLeave = false },
        )
    }
}

@Composable
private fun PreferenceToggle(title: String, checked: Boolean, enabled: Boolean, onChange: (Boolean) -> Unit) {
    Row(
        modifier = Modifier.fillMaxWidth().heightIn(min = 48.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Text(title, modifier = Modifier.weight(1f))
        Switch(checked = checked, onCheckedChange = onChange, enabled = enabled)
    }
}

@Composable
private fun PeerCard(peer: GroupPeer, enabled: Boolean) {
    val address = normalizeAddress(peer.holding)
    val local = sameAddress(SessionState.holding, address)
    WorkspaceCard(peer.displayName, "${peer.platform.ifBlank { "未知平台" }} / ${peer.appVersion.ifBlank { "版本未知" }}") {
        SelectionContainer {
            Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                Text(address?.let { "持有耳机: $it" } ?: "当前没有持有耳机")
                Text("成员 ID: ${peer.id}", style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        }
        if (!peer.canAudio) {
            Text("该成员未声明音频交接能力.", color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        FilledTonalButton(
            onClick = { AppActions.claimPeer(peer.id) },
            enabled = enabled && peer.canAudio && address != null && !local,
            modifier = Modifier.heightIn(min = 48.dp),
        ) {
            Text(when {
                local -> "本机已持有这副耳机"
                !peer.canAudio -> "暂不支持交接"
                address == null -> "没有可接管的耳机"
                else -> "接管到这台 Android"
            })
        }
    }
}

@Composable
private fun HandoffCard(progress: HandoffProgress, onBluetoothSettings: () -> Unit) {
    WorkspaceCard(progress.title, progress.detail) {
        if (progress.active) LinearProgressIndicator(modifier = Modifier.fillMaxWidth())
        FlowRow(horizontalArrangement = Arrangement.spacedBy(12.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            listOf("发起请求", "释放原连接", "建立新连接", "完成").forEachIndexed { index, label ->
                Text(
                    "${index + 1}. $label",
                    color = if (progress.kind != "failed" && index <= progress.step) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant,
                    style = MaterialTheme.typography.labelLarge,
                )
            }
        }
        if (progress.kind == "failed" || (!progress.active && progress.kind != "done")) {
            HorizontalDivider()
            Text("确认耳机已开启并在两端配对, 原设备客户端仍在运行. 若原连接未释放, 先暂停其播放; 若本机未接通音频, 请从系统蓝牙设置连接后重试.")
            OutlinedButton(onClick = onBluetoothSettings, modifier = Modifier.heightIn(min = 48.dp)) { Text("打开蓝牙设置") }
        }
    }
}
