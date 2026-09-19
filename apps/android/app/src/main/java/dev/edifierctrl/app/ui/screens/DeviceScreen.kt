@file:OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)

package dev.edifierctrl.app.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.unit.dp
import dev.edifierctrl.app.infrastructure.AppPreferences
import dev.edifierctrl.app.session.AppActions
import dev.edifierctrl.app.session.GroupPeer
import dev.edifierctrl.app.session.HeadphoneDevice
import dev.edifierctrl.app.session.SessionState
import dev.edifierctrl.app.session.normalizeAddress
import dev.edifierctrl.app.session.sameAddress
import dev.edifierctrl.app.ui.components.ConnectionSummary
import dev.edifierctrl.app.ui.components.EmptyMessage
import dev.edifierctrl.app.ui.components.SectionDisclosure
import dev.edifierctrl.app.ui.components.WorkspaceCard
import dev.edifierctrl.app.ui.components.WorkspacePage

private data class DeviceListing(
    val device: HeadphoneDevice,
    val holder: GroupPeer?,
    val connected: Boolean,
    val heldLocally: Boolean,
    val lastUsed: Boolean,
)

@Composable
fun DeviceScreen(onHandoff: () -> Unit = {}, onBluetoothSettings: () -> Unit = {}) {
    var manualAddress by rememberSaveable { mutableStateOf("") }
    val normalizedManual = normalizeAddress(manualAddress)
    val listings = deviceListings()
    val enabled = SessionState.ready && !SessionState.busy

    WorkspacePage("设备", "选择已配对耳机, 或将另一台设备的耳机交接到这台 Android.") {
        ConnectionSummary()
        WorkspaceCard("发现耳机", "已配对设备, 上次使用的耳机和组内已知耳机会合并显示.") {
            FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                FilledTonalButton(
                    onClick = { AppActions.scan("rfcomm") },
                    enabled = enabled,
                    modifier = Modifier.heightIn(min = 48.dp),
                ) { Text("扫描已配对耳机") }
                OutlinedButton(onClick = onBluetoothSettings, modifier = Modifier.heightIn(min = 48.dp)) { Text("系统蓝牙设置") }
                if (SessionState.connected) {
                    OutlinedButton(
                        onClick = { AppActions.disconnect() },
                        enabled = enabled,
                        modifier = Modifier.heightIn(min = 48.dp),
                    ) { Text("断开控制") }
                }
            }
            Text(
                "使用经典蓝牙连接, 新耳机请先在系统蓝牙中配对.",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        if (listings.isEmpty()) {
            EmptyMessage("还没有找到耳机", "完成系统配对后点击扫描. 也可以加入交接组, 查看其他设备正在使用的耳机.")
        } else {
            Text("可见耳机 (${listings.size})", style = MaterialTheme.typography.titleMedium)
            listings.forEach { listing ->
                key(listing.device.address) {
                    DeviceCard(listing, enabled, onHandoff)
                }
            }
        }
        SectionDisclosure("手工输入蓝牙地址", "用于已配对但未出现在列表中的耳机. 地址格式为 AA:BB:CC:DD:EE:FF.") {
            OutlinedTextField(
                value = manualAddress,
                onValueChange = { manualAddress = it },
                label = { Text("耳机 MAC 地址") },
                supportingText = { Text(if (manualAddress.isNotBlank() && normalizedManual == null) "请输入 12 位十六进制地址, 可带冒号或短横线." else "系统蓝牙配对是建立控制通道的前提.") },
                isError = manualAddress.isNotBlank() && normalizedManual == null,
                enabled = enabled,
                singleLine = true,
                keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Characters),
                modifier = Modifier.fillMaxWidth(),
            )
            val holder = normalizedManual?.let(::peerHolding)
            FilledTonalButton(
                onClick = {
                    normalizedManual?.let { address ->
                        if (holder != null) {
                            AppActions.claimPeer(holder.id)
                            onHandoff()
                        } else {
                            AppActions.connect(address)
                        }
                    }
                },
                enabled = enabled && normalizedManual != null && (holder == null || (SessionState.groupJoined && holder.canAudio)),
                modifier = Modifier.heightIn(min = 48.dp),
            ) { Text(if (holder != null) "从 ${holder.displayName} 接管" else "连接控制通道") }
        }
    }
}

@Composable
private fun DeviceCard(listing: DeviceListing, enabled: Boolean, onHandoff: () -> Unit) {
    val holder = listing.holder
    val supported = listing.device.kind == "rfcomm"
    val canAct = enabled && (!listing.connected || holder != null) && supported &&
        (holder == null || (holder.canAudio && SessionState.groupJoined))
    val action = when {
        !supported -> "当前 Android 不支持此连接方式"
        holder != null && !holder.canAudio -> "该成员不支持音频交接"
        holder != null -> "从 ${holder.displayName} 接管"
        listing.connected -> "控制通道已连接"
        listing.heldLocally -> "连接本机耳机控制"
        else -> "连接控制通道"
    }
    Card(
        onClick = {
            if (holder != null) {
                AppActions.claimPeer(holder.id)
                onHandoff()
            } else {
                AppActions.connect(listing.device.address, listing.device.name, "rfcomm")
            }
        },
        enabled = canAct,
        colors = CardDefaults.cardColors(),
        modifier = Modifier.fillMaxWidth().heightIn(min = 48.dp),
    ) {
        Column(Modifier.padding(18.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(listing.device.displayName, style = MaterialTheme.typography.titleMedium)
            Text(listing.device.address, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
            Text(
                buildList {
                    if (listing.lastUsed) add("上次使用")
                    if (listing.heldLocally) add("本机已确认持有音频")
                    if (holder != null) add("${holder.displayName} 正在持有")
                    if (isEmpty()) add("已知耳机")
                }.joinToString(" / "),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Text(action, style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.primary)
        }
    }
}

private fun peerHolding(address: String): GroupPeer? {
    if (sameAddress(SessionState.holding, address)) return null
    return SessionState.peers.firstOrNull { sameAddress(it.holding, address) }
}

private fun deviceListings(): List<DeviceListing> {
    val devices = linkedMapOf<String, HeadphoneDevice>()
    fun add(device: HeadphoneDevice) {
        val address = normalizeAddress(device.address) ?: return
        val existing = devices[address]
        if (existing == null || (existing.name.isBlank() && device.name.isNotBlank())) {
            devices[address] = device.copy(address = address)
        }
    }
    SessionState.devices.forEach(::add)
    normalizeAddress(AppPreferences.lastDeviceAddress)?.let {
        add(HeadphoneDevice(it, AppPreferences.lastDeviceName))
    }
    SessionState.address?.let { add(HeadphoneDevice(it, SessionState.deviceName.orEmpty())) }
    SessionState.holding?.let { add(HeadphoneDevice(it, "本机正在使用的耳机")) }
    SessionState.peers.forEach { peer ->
        peer.holding?.let { add(HeadphoneDevice(it, "${peer.displayName} 的耳机")) }
    }
    return devices.values.map { device ->
        DeviceListing(
            device = device,
            holder = peerHolding(device.address),
            connected = SessionState.connected && sameAddress(SessionState.address, device.address),
            heldLocally = sameAddress(SessionState.holding, device.address),
            lastUsed = sameAddress(AppPreferences.lastDeviceAddress, device.address),
        )
    }.sortedWith(compareByDescending<DeviceListing> { it.connected }.thenByDescending { it.lastUsed }.thenBy { it.device.displayName.lowercase() })
}
