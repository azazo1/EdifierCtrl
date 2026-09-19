package dev.edifierctrl.app.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.WindowInsetsSides
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.only
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.Bluetooth
import androidx.compose.material.icons.outlined.Groups
import androidx.compose.material.icons.outlined.Headphones
import androidx.compose.material.icons.outlined.History
import androidx.compose.material.icons.outlined.Settings
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarDefaults
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.Icon
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.currentBackStackEntryAsState
import androidx.navigation.compose.rememberNavController
import dev.edifierctrl.app.session.AppActions
import dev.edifierctrl.app.session.SessionState

@Composable
fun WorkspaceApp(onRequestPermission: () -> Unit, onBluetoothSettings: () -> Unit, onAppSettings: () -> Unit) {
    val nav = rememberNavController()
    fun navigate(route: String) {
        nav.navigate(route) {
            popUpTo("control") { saveState = true }
            launchSingleTop = true
            restoreState = true
        }
    }
    val tabs = listOf(
        Triple("control", "耳机", Icons.Outlined.Headphones),
        Triple("device", "设备", Icons.Outlined.Bluetooth),
        Triple("group", "交接", Icons.Outlined.Groups),
        Triple("settings", "设置", Icons.Outlined.Settings),
        Triple("activity", "活动", Icons.Outlined.History),
    )
    Scaffold(
        modifier = Modifier.fillMaxSize(),
        containerColor = MaterialTheme.colorScheme.background,
        contentWindowInsets = WindowInsets.safeDrawing.only(WindowInsetsSides.Horizontal + WindowInsetsSides.Top),
        bottomBar = {
            val route = nav.currentBackStackEntryAsState().value?.destination?.route
            NavigationBar(windowInsets = NavigationBarDefaults.windowInsets) {
                tabs.forEach { (id, label, icon) ->
                    NavigationBarItem(
                        selected = route == id, onClick = { navigate(id) },
                        icon = { Icon(icon, contentDescription = null) }, label = { Text(label) },
                    )
                }
            }
        },
    ) { inner ->
        Column(Modifier.fillMaxSize().padding(inner)) {
            if (!SessionState.permissionsGranted) {
                PermissionNotice(onRequestPermission, onAppSettings)
            }
            SessionState.operation?.let { operation ->
                Text(operation, Modifier.padding(horizontal = 20.dp, vertical = 8.dp), style = MaterialTheme.typography.labelLarge)
                LinearProgressIndicator(modifier = Modifier.fillMaxWidth())
            }
            SessionState.notice?.let { notice ->
                Card(
                    modifier = Modifier.fillMaxWidth().padding(horizontal = 20.dp, vertical = 6.dp),
                    colors = CardDefaults.cardColors(containerColor = if (notice.isError) MaterialTheme.colorScheme.errorContainer else MaterialTheme.colorScheme.secondaryContainer),
                ) {
                    Row(Modifier.padding(start = 14.dp, top = 8.dp, bottom = 8.dp, end = 4.dp), verticalAlignment = Alignment.Top) {
                        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                            Text(notice.title, style = MaterialTheme.typography.titleSmall)
                            if (notice.detail.isNotBlank()) {
                                Text(notice.detail, style = MaterialTheme.typography.bodySmall, maxLines = 3, overflow = TextOverflow.Ellipsis)
                            }
                            if (notice.isError) {
                                TextButton(onClick = { navigate("activity") }) { Text("查看完整记录") }
                            }
                        }
                        TextButton(onClick = AppActions::clearNotice) { Text("关闭") }
                    }
                }
            }
            NavHost(navController = nav, startDestination = "control", modifier = Modifier.weight(1f)) {
                composable("control") { ControlScreen(onDevices = { navigate("device") }, onHandoff = { navigate("group") }) }
                composable("device") { DeviceScreen(onHandoff = { navigate("group") }, onBluetoothSettings = onBluetoothSettings) }
                composable("group") { GroupScreen(onBluetoothSettings = onBluetoothSettings) }
                composable("settings") { SettingsScreen(onBluetoothSettings, onAppSettings) }
                composable("activity") { ActivityScreen() }
            }
        }
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun PermissionNotice(onRequestPermission: () -> Unit, onAppSettings: () -> Unit) {
    Card(Modifier.fillMaxWidth().padding(horizontal = 20.dp, vertical = 6.dp)) {
        Column(Modifier.padding(14.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
            Text("允许访问蓝牙设备", style = MaterialTheme.typography.titleSmall)
            Text("连接已配对耳机需要附近设备权限; 较旧 Android 使用位置权限授权蓝牙访问.", style = MaterialTheme.typography.bodySmall)
            FlowRow(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                TextButton(onClick = onRequestPermission) { Text("授予权限") }
                TextButton(onClick = onAppSettings) { Text("应用设置") }
            }
        }
    }
}
