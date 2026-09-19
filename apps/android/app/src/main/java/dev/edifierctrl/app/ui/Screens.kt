package dev.edifierctrl.app.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
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
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import dev.edifierctrl.app.EdifierNative
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

@Composable
fun StatusBanner() {
    Card(
        modifier = Modifier.fillMaxWidth(),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceVariant),
    ) {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            Text(SessionUi.statusLine(), style = MaterialTheme.typography.titleSmall)
            Text(
                SessionUi.hint,
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            SessionUi.battery?.let { pct ->
                LinearProgressIndicator(
                    progress = { pct / 100f },
                    modifier = Modifier.fillMaxWidth(),
                )
            }
        }
    }
}

@Composable
fun DeviceScreen() {
    var scanned by remember { mutableStateOf(listOf<DeviceRow>()) }
    var kind by remember { mutableStateOf("rfcomm") }
    var scanning by remember { mutableStateOf(false) }
    val session = remember { EdifierNative.ensureSession() }
    val scope = rememberCoroutineScope()
    val devices = decorateDevices(scanned)
    fun scanNow() {
        scanning = true
        scope.launch {
            val json = withContext(Dispatchers.IO) {
                nativeCall { EdifierNative.sessionScan(session, kind) }
            }
            scanned = parseDevices(json)
            val listed = decorateDevices(scanned)
            SessionUi.hint = if (listed.isEmpty()) {
                "没有发现漫步者耳机. 请先在系统蓝牙里配对."
            } else {
                "找到 ${listed.size} 台."
            }
            scanning = false
        }
    }
    LaunchedEffect(kind) {
        scanNow()
    }
    Column(Modifier.fillMaxSize().padding(horizontal = 20.dp, vertical = 12.dp)) {
        Text("设备", style = MaterialTheme.typography.headlineSmall)
        Text(
            "只列出已配对的漫步者耳机. 系统已连上时会自动接管控制通道, 被其他主机占用时可点卡片请求接管.",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(top = 4.dp, bottom = 12.dp),
        )
        StatusBanner()
        Spacer(Modifier.height(12.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            FilterChip(
                selected = kind == "rfcomm",
                onClick = { kind = "rfcomm" },
                label = { Text("经典蓝牙") },
            )
            FilterChip(
                selected = kind == "ble",
                onClick = { kind = "ble" },
                label = { Text("BLE") },
            )
        }
        Spacer(Modifier.height(8.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Button(
                onClick = { scanNow() },
            ) {
                Text(if (scanning) "扫描中" else "扫描")
            }
            OutlinedButton(
                enabled = SessionUi.connected,
                onClick = {
                    SessionUi.hint = nativeCall {
                        val rc = EdifierNative.sessionDisconnect(session)
                        if (rc != 0) EdifierNative.lastError() else "已断开控制通道"
                    }
                    SessionUi.connected = false
                },
            ) {
                Text("断开")
            }
        }
        Spacer(Modifier.height(12.dp))
        LazyColumn(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            items(devices, key = { it.address }) { row ->
                Card(
                    modifier = Modifier.fillMaxWidth(),
                    onClick = {
                        val name = row.name.ifBlank { row.address }
                        val holderId = row.holderId
                        if (holderId != null) {
                            SessionUi.hint = nativeCall {
                                val rc = EdifierNative.sessionGroupClaimPeer(session, holderId)
                                if (rc != 0) {
                                    EdifierNative.lastError()
                                } else {
                                    "已向 ${row.holderHost ?: holderId} 请求接管"
                                }
                            }
                            return@Card
                        }
                        SessionUi.hint = "正在连接 $name"
                        scope.launch {
                            val result = withContext(Dispatchers.IO) {
                                nativeCall {
                                    val rc = EdifierNative.sessionConnect(session, row.address, kind)
                                    if (rc != 0) {
                                        EdifierNative.lastError()
                                    } else {
                                        EdifierNative.sessionSendJson(session, """{"op":"query_battery"}""")
                                        "已连接 $name"
                                    }
                                }
                            }
                            if (result.startsWith("已连接")) {
                                SessionUi.address = row.address
                                SessionUi.deviceName = name
                                SessionUi.connected = true
                            }
                            SessionUi.hint = result
                        }
                    },
                ) {
                    Column(Modifier.padding(16.dp)) {
                        Text(
                            row.name.ifBlank { "未命名耳机" },
                            style = MaterialTheme.typography.titleMedium,
                        )
                        Text(
                            row.address,
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                        Text(
                            row.action,
                            style = MaterialTheme.typography.labelMedium,
                            color = MaterialTheme.colorScheme.primary,
                            modifier = Modifier.padding(top = 6.dp),
                        )
                    }
                }
            }
        }
    }
}

@Composable
fun GroupScreen() {
    val context = LocalContext.current
    var pass by remember { mutableStateOf(savedPassphrase(context)) }
    var advanced by remember { mutableStateOf(false) }
    var mac by remember { mutableStateOf("") }
    val session = remember { EdifierNative.ensureSession() }
    val peers = SessionUi.peers
    Column(Modifier.fillMaxSize().padding(horizontal = 20.dp, vertical = 12.dp)) {
        Text("组", style = MaterialTheme.typography.headlineSmall)
        Text(
            "同一口令的电脑和手机在同一局域网. 启动后会自动加入上次的组. 点成员即可接管它正在用的耳机.",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(top = 4.dp, bottom = 12.dp),
        )
        StatusBanner()
        Spacer(Modifier.height(12.dp))
        OutlinedTextField(
            value = pass,
            onValueChange = { pass = it },
            label = { Text("组口令") },
            modifier = Modifier.fillMaxWidth(),
            singleLine = true,
        )
        Spacer(Modifier.height(8.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
            Button(
                onClick = {
                    savePassphrase(context, pass)
                    SessionUi.hint = joinGroup(session, pass)
                },
            ) { Text(if (SessionUi.groupJoined) "已加入" else "加入") }
            OutlinedButton(onClick = { SessionUi.peers = loadPeers(session) }) { Text("刷新") }
        }
        Spacer(Modifier.height(12.dp))
        if (peers.isEmpty()) {
            Text(
                if (SessionUi.groupJoined) "还没有其他成员. 确认电脑和手机在同一 Wi-Fi." else "加入后这里会列出局域网里的其他设备.",
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        LazyColumn(verticalArrangement = Arrangement.spacedBy(8.dp), modifier = Modifier.weight(1f, fill = false)) {
            items(peers, key = { it.id }) { peer ->
                Card(modifier = Modifier.fillMaxWidth()) {
                    Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                        Text(peer.host, style = MaterialTheme.typography.titleMedium)
                        Text(
                            peer.id,
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                        Text(
                            if (peer.holding.isNullOrEmpty()) "未持有耳机" else "持有 ${peer.holding}",
                            style = MaterialTheme.typography.bodyMedium,
                        )
                        Button(
                            enabled = !peer.holding.isNullOrEmpty(),
                            onClick = {
                                SessionUi.hint = nativeCall {
                                    val rc = EdifierNative.sessionGroupClaimPeer(session, peer.id)
                                    if (rc != 0) {
                                        EdifierNative.lastError()
                                    } else {
                                        "已向 ${peer.host} 请求接管"
                                    }
                                }
                            },
                        ) {
                            Text(if (peer.holding.isNullOrEmpty()) "无法接管" else "接管音频")
                        }
                    }
                }
            }
        }
        TextButton(onClick = { advanced = !advanced }) {
            Text(if (advanced) "收起高级" else "用 MAC 接管")
        }
        if (advanced) {
            OutlinedTextField(
                value = mac,
                onValueChange = { mac = it },
                label = { Text("耳机 MAC") },
                modifier = Modifier.fillMaxWidth(),
                singleLine = true,
            )
            Spacer(Modifier.height(8.dp))
            OutlinedButton(onClick = {
                SessionUi.hint = nativeCall {
                    val rc = EdifierNative.sessionGroupClaim(session, mac)
                    if (rc != 0) EdifierNative.lastError() else "已请求接管 $mac"
                }
            }) { Text("接管") }
        }
    }
}

@Composable
fun DebugScreen() {
    var payload by remember { mutableStateOf("""{"op":"query_battery"}""") }
    var log by remember { mutableStateOf("") }
    Column(
        Modifier
            .fillMaxSize()
            .padding(horizontal = 20.dp, vertical = 12.dp)
            .verticalScroll(rememberScrollState()),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Text("调试", style = MaterialTheme.typography.headlineSmall)
        Text(
            "封装命令或解析原始帧. 日常使用不用进这一页.",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        OutlinedTextField(
            value = payload,
            onValueChange = { payload = it },
            label = { Text("JSON 或 hex") },
            modifier = Modifier.fillMaxWidth(),
            minLines = 3,
        )
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            FilledTonalButton(onClick = { log = encode(payload) }) { Text("封装命令") }
            FilledTonalButton(onClick = {
                log = nativeCall { EdifierNative.frameParse(payload) }
            }) { Text("解析帧") }
        }
        if (log.isNotEmpty()) {
            Card(modifier = Modifier.fillMaxWidth()) {
                Text(log, Modifier.padding(16.dp), style = MaterialTheme.typography.bodySmall)
            }
        }
    }
}

@Composable
fun EventPump() {
    val context = LocalContext.current
    val session = remember { EdifierNative.ensureSession() }
    LaunchedEffect(session) {
        if (session == 0L || !EdifierNative.loaded) {
            return@LaunchedEffect
        }
        val pass = savedPassphrase(context)
        if (pass.isNotBlank() && !SessionUi.groupJoined) {
            val msg = withContext(Dispatchers.IO) { joinGroup(session, pass) }
            SessionUi.hint = if (SessionUi.groupJoined) "已自动加入组" else msg
        }
        withContext(Dispatchers.IO) { autoConnectLinked(session) }
        var n = 0
        while (true) {
            delay(250)
            n += 1
            if (SessionUi.groupJoined && n % 8 == 0) {
                SessionUi.peers = withContext(Dispatchers.IO) { loadPeers(session) }
            }
            val ev = runCatching { EdifierNative.sessionPollEvent(session) }.getOrNull() ?: continue
            if (ev.contains("\"empty\"")) {
                continue
            }
            SessionUi.applyEvent(ev)
        }
    }
}

private data class ListedDevice(
    val address: String,
    val name: String,
    val holderId: String?,
    val holderHost: String?,
    val action: String,
)

private fun decorateDevices(scanned: List<DeviceRow>): List<ListedDevice> {
    val peers = SessionUi.peers
    val local = SessionUi.holding
    val map = linkedMapOf<String, ListedDevice>()
    fun key(addr: String) = addr.filter { it.isLetterOrDigit() }.uppercase()
    for (row in scanned) {
        val holder = holderOf(row.address, peers)
        val self = sameMac(local, row.address)
        val occupied = holder != null && !self
        map[key(row.address)] = ListedDevice(
            address = row.address,
            name = row.name,
            holderId = if (occupied) holder?.id else null,
            holderHost = if (occupied) holder?.host else null,
            action = when {
                occupied -> "被 ${holder?.host} 占用 · 点按接管"
                self -> "本机持有 · 点按连接"
                else -> "点按连接"
            },
        )
    }
    for (peer in peers) {
        val mac = peer.holding ?: continue
        if (sameMac(local, mac) || map.containsKey(key(mac))) {
            continue
        }
        map[key(mac)] = ListedDevice(
            address = mac,
            name = "占用中的耳机",
            holderId = peer.id,
            holderHost = peer.host,
            action = "被 ${peer.host} 占用 · 点按接管",
        )
    }
    return map.values.toList()
}
