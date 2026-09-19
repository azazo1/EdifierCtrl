package dev.edifierctrl.app.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import dev.edifierctrl.app.EdifierNative
import kotlinx.coroutines.delay
import org.json.JSONArray

@Composable
fun DeviceScreen() {
    var log by remember { mutableStateOf(statusLine()) }
    var devices by remember { mutableStateOf(listOf<DeviceRow>()) }
    val session = remember { EdifierNative.ensureSession() }
    Column(Modifier.padding(24.dp).verticalScroll(rememberScrollState())) {
        Text("设备")
        Text(log)
        Button(onClick = {
            val json = nativeCall { EdifierNative.sessionScan(session, "rfcomm") }
            log = json
            devices = parseDevices(json)
        }) {
            Text("扫描 RFCOMM")
        }
        Button(onClick = {
            val json = nativeCall { EdifierNative.sessionScan(session, "ble") }
            log = json
            devices = parseDevices(json)
        }) {
            Text("扫描 BLE")
        }
        devices.forEach { row ->
            Text(
                "${row.address}  ${row.name}",
                modifier = Modifier.clickable {
                    log = nativeCall {
                        val rc = EdifierNative.sessionConnect(session, row.address, "rfcomm")
                        if (rc != 0) EdifierNative.lastError() else "已连接 ${row.address}"
                    }
                },
            )
        }
        Button(onClick = {
            log = nativeCall {
                val rc = EdifierNative.sessionDisconnect(session)
                if (rc != 0) EdifierNative.lastError() else "已断开控制通道"
            }
        }) {
            Text("断开控制")
        }
        EventLog(session) { ev -> log = ev + "\n" + log }
    }
}

@Composable
fun ControlScreen() {
    var log by remember { mutableStateOf("先连接耳机.") }
    var confirmCd by remember { mutableStateOf(false) }
    val session = remember { EdifierNative.ensureSession() }
    Column(Modifier.padding(24.dp).verticalScroll(rememberScrollState())) {
        Text("控制")
        Button(onClick = { log = sendOrEncode(session, """{"op":"set_noise_mode","mode":"normal"}""") }) {
            Text("降噪关")
        }
        Button(onClick = { log = sendOrEncode(session, """{"op":"set_noise_mode","mode":"reduction"}""") }) {
            Text("降噪")
        }
        Button(onClick = { log = sendOrEncode(session, """{"op":"query_battery"}""") }) {
            Text("查电量")
        }
        Button(onClick = {
            log = nativeCall {
                val rc = EdifierNative.sessionReadout(session, "basedevice")
                if (rc != 0) EdifierNative.lastError() else "已发送读状态"
            }
        }) {
            Text("读取状态")
        }
        Button(onClick = { confirmCd = true }) { Text("断开主机 (CD)") }
        if (confirmCd) {
            AlertDialog(
                onDismissRequest = { confirmCd = false },
                title = { Text("确认") },
                text = { Text("发 CD 会断开当前主机. 交接回退可以自动发, 这里是手动.") },
                confirmButton = {
                    Button(onClick = {
                        confirmCd = false
                        log = sendOrEncode(session, """{"op":"disconnect_host"}""")
                    }) { Text("发送") }
                },
                dismissButton = {
                    Button(onClick = { confirmCd = false }) { Text("取消") }
                },
            )
        }
        EventLog(session) { ev -> log = ev + "\n" + log }
        Text(log)
    }
}

@Composable
fun GroupScreen() {
    var pass by remember { mutableStateOf("") }
    var mac by remember { mutableStateOf("") }
    var log by remember { mutableStateOf("加入后点某个成员接管其耳机.") }
    var peers by remember { mutableStateOf(listOf<PeerRow>()) }
    val session = remember { EdifierNative.ensureSession() }
    Column(Modifier.padding(24.dp).verticalScroll(rememberScrollState())) {
        Text("组")
        OutlinedTextField(value = pass, onValueChange = { pass = it }, label = { Text("组口令") })
        Button(onClick = {
            log = nativeCall {
                val rc = EdifierNative.sessionGroupJoin(session, pass)
                if (rc != 0) EdifierNative.lastError() else "已加入"
            }
            peers = loadPeers(session)
        }) {
            Text("加入")
        }
        Button(onClick = { peers = loadPeers(session) }) { Text("刷新成员") }
        peers.forEach { peer ->
            Text(
                peer.label,
                modifier = Modifier.clickable {
                    log = nativeCall {
                        val rc = EdifierNative.sessionGroupClaimPeer(session, peer.id)
                        if (rc != 0) EdifierNative.lastError() else "已请求接管 ${peer.label}"
                    }
                },
            )
        }
        OutlinedTextField(value = mac, onValueChange = { mac = it }, label = { Text("耳机 MAC") })
        Button(onClick = {
            log = nativeCall {
                val rc = EdifierNative.sessionGroupClaim(session, mac)
                if (rc != 0) EdifierNative.lastError() else "已请求接管 $mac"
            }
        }) {
            Text("接管音频")
        }
        Text(log)
    }
}

@Composable
fun DebugScreen() {
    var payload by remember { mutableStateOf("""{"op":"query_battery"}""") }
    var log by remember { mutableStateOf("") }
    Column(Modifier.padding(24.dp).verticalScroll(rememberScrollState())) {
        Text("调试")
        OutlinedTextField(value = payload, onValueChange = { payload = it }, label = { Text("JSON 或 hex") })
        Button(onClick = { log = encode(payload) }) { Text("封装命令") }
        Button(onClick = { log = nativeCall { EdifierNative.frameParse(payload) } }) { Text("解析帧") }
        Text(log)
    }
}

private data class DeviceRow(val address: String, val name: String)

private data class PeerRow(val id: String, val label: String)

private fun parseDevices(json: String): List<DeviceRow> {
    return runCatching {
        val arr = JSONArray(json)
        (0 until arr.length()).map { i ->
            val o = arr.getJSONObject(i)
            DeviceRow(o.optString("address"), o.optString("name"))
        }
    }.getOrDefault(emptyList())
}

@Composable
private fun EventLog(session: Long, onEvent: (String) -> Unit) {
    LaunchedEffect(session) {
        if (session == 0L || !EdifierNative.loaded) {
            return@LaunchedEffect
        }
        while (true) {
            delay(250)
            val ev = runCatching { EdifierNative.sessionPollEvent(session) }.getOrNull() ?: continue
            if (ev.contains("empty")) {
                continue
            }
            onEvent(ev)
        }
    }
}

private fun statusLine(): String {
    if (!EdifierNative.loaded) {
        return "尚未加载 libedifier_ffi.so"
    }
    return runCatching { "核心库 " + EdifierNative.version() }.getOrElse { it.message ?: "FFI 失败" }
}

private fun encode(json: String): String {
    if (!EdifierNative.loaded) {
        return "尚未加载 libedifier_ffi.so"
    }
    return EdifierNative.commandEncode(json) ?: (EdifierNative.lastError() ?: "编码失败")
}

private fun sendOrEncode(session: Long, json: String): String {
    if (!EdifierNative.loaded || session == 0L) {
        return encode(json)
    }
    val rc = runCatching { EdifierNative.sessionSendJson(session, json) }.getOrDefault(-1)
    return if (rc == 0) "已发送 $json" else encode(json)
}

private fun loadPeers(session: Long): List<PeerRow> {
    val json = nativeCall { EdifierNative.sessionGroupPeers(session) }
    return runCatching {
        val arr = JSONArray(json)
        (0 until arr.length()).map { i ->
            val o = arr.getJSONObject(i)
            val id = o.optString("id")
            val host = o.optString("hostname", id)
            val holding = if (o.isNull("holding")) null else o.optString("holding")
            PeerRow(id, if (holding.isNullOrEmpty()) "$host  未持有" else "$host  holding=$holding")
        }
    }.getOrDefault(emptyList())
}

private fun nativeCall(block: () -> String?): String {
    if (!EdifierNative.loaded) {
        return "尚未加载 libedifier_ffi.so"
    }
    return runCatching { block() ?: (EdifierNative.lastError() ?: "") }.getOrElse { it.message ?: "FFI 失败" }
}
