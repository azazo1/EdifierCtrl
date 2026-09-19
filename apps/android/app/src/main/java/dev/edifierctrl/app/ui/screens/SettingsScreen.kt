package dev.edifierctrl.app.ui

import android.os.Build
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.material3.Button
import androidx.compose.material3.FilterChip
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import dev.edifierctrl.app.infrastructure.AppPreferences
import dev.edifierctrl.app.session.AppActions
import dev.edifierctrl.app.session.SessionState
import dev.edifierctrl.app.ui.components.ConfirmAction
import dev.edifierctrl.app.ui.components.WorkspaceCard
import dev.edifierctrl.app.ui.components.WorkspacePage

@OptIn(ExperimentalLayoutApi::class)
@Composable
fun SettingsScreen(onBluetoothSettings: () -> Unit, onAppSettings: () -> Unit) {
    var confirmStop by remember { mutableStateOf(false) }
    val context = LocalContext.current
    val version = remember(context) {
        runCatching { context.packageManager.getPackageInfo(context.packageName, 0).versionName }
            .getOrNull() ?: "开发版本"
    }
    val idle = !SessionState.busy
    WorkspacePage("应用设置", "按你的使用习惯安排外观和交接组.") {
        WorkspaceCard("外观", "与桌面端保持一致的蓝色工作台, 适配系统明暗模式.") {
            FlowRow(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                listOf("system" to "跟随系统", "light" to "浅色", "dark" to "深色").forEach { (key, label) ->
                    FilterChip(
                        selected = AppPreferences.theme == key,
                        onClick = { AppPreferences.setTheme(key) },
                        label = { Text(label) },
                    )
                }
            }
        }
        WorkspaceCard("启动与交接", "组名需要在手机与电脑之间完全一致.") {
            SettingToggle("记住组名", "下次打开交接页时填入已保存的组名.",
                AppPreferences.rememberGroup, idle, AppPreferences::setRememberGroup)
            SettingToggle("启动时自动加入", "蓝牙权限就绪后加入已保存的交接组.",
                AppPreferences.autoJoinGroup, idle && AppPreferences.rememberGroup, AppPreferences::setAutoJoinGroup)
            if (AppPreferences.rememberGroup && AppPreferences.groupName.isNotEmpty()) {
                SelectionContainer { Text("已保存组名: ${AppPreferences.groupName}") }
            }
            if (SessionState.groupJoined) {
                Text("本次仍在组内, 更改记忆设置不会退出当前组.", style = MaterialTheme.typography.bodySmall)
            }
            AppPreferences.saveError?.let {
                Text(it, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall)
            }
        }
        WorkspaceCard("耳机服务", if (SessionState.ready) "服务运行中" else "服务尚未就绪") {
            Text("切换页面或旋转屏幕不会中断会话. 应用进程被系统结束后交接会停止; 需要接管时请保持两端应用运行.",
                style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
            if (SessionState.ready) {
                OutlinedButton(onClick = { confirmStop = true }, enabled = idle) { Text("停止耳机服务") }
            } else {
                Button(onClick = AppActions::start, enabled = idle && SessionState.permissionsGranted && SessionState.nativeAvailable) {
                    Text("启动耳机服务")
                }
            }
            FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                TextButton(onClick = onBluetoothSettings) { Text("系统蓝牙设置") }
                TextButton(onClick = onAppSettings) { Text("应用权限设置") }
            }
        }
        WorkspaceCard("关于 EdifierCtrl", "声音, 随你而行.") {
            Text("应用版本: $version")
            Text("协议核心: ${SessionState.coreVersion.ifEmpty { "尚未加载" }}")
            Text("Android ${Build.VERSION.RELEASE}", color = MaterialTheme.colorScheme.onSurfaceVariant)
            Text("音频接管能力取决于 Android 系统和厂商的蓝牙限制. 控制通道与系统音频分别确认.",
                style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
    }
    if (confirmStop) {
        ConfirmAction("停止耳机服务", "将退出交接组并关闭控制通道. 你可以稍后在此重新启动.", "停止服务", {
            confirmStop = false
            AppActions.stop()
        }, { confirmStop = false })
    }
}

@Composable
private fun SettingToggle(title: String, detail: String, checked: Boolean, enabled: Boolean, onChange: (Boolean) -> Unit) {
    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(4.dp)) {
            Text(title, style = MaterialTheme.typography.titleSmall)
            Text(detail, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        Switch(checked = checked, onCheckedChange = onChange, enabled = enabled)
    }
}
