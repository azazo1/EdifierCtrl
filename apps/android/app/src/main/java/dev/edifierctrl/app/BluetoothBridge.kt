package dev.edifierctrl.app

import android.app.Application
import android.bluetooth.BluetoothA2dp
import android.bluetooth.BluetoothAdapter
import android.bluetooth.BluetoothDevice
import android.bluetooth.BluetoothProfile
import android.bluetooth.BluetoothSocket
import android.content.Context
import android.net.wifi.WifiManager
import android.util.Log
import org.json.JSONArray
import org.json.JSONObject
import java.util.UUID

/**
 * 给 Rust `edifier-bt-android` 调的 RFCOMM / A2DP 桥.
 * A2DP connect 在 Android 14 起是隐藏 API, 失败时交接会走 CD.
 */
object BluetoothBridge {
    private const val TAG = "EdifierBt"
    private const val RFCOMM = "edf00000-edfe-dfed-fedf-edfedfedfedf"

    @Volatile
    private var app: Application? = null

    @Volatile
    private var a2dp: BluetoothA2dp? = null

    @Volatile
    private var socket: BluetoothSocket? = null

    @Volatile
    private var lastErrorText: String = ""

    @Volatile
    private var multicastLock: WifiManager.MulticastLock? = null

    @JvmStatic
    fun lastError(): String = lastErrorText

    @Volatile
    private var started = false

    private var generation = 0

    @JvmStatic
    fun attach(application: Application) {
        // Application 创建时还没有运行时权限, 这里只保存上下文.
        app = application
    }

    @JvmStatic
    @Synchronized
    fun start() {
        if (started) return
        val application = checkNotNull(app) { "蓝牙桥尚未初始化" }
        val bluetooth = checkNotNull(adapter()) { "手机没有蓝牙适配器" }
        check(bluetooth.isEnabled) { "请先在系统设置中打开蓝牙" }
        val currentGeneration = ++generation
        started = true
        try {
            val wifi = application.getSystemService(Context.WIFI_SERVICE) as WifiManager
            multicastLock = wifi.createMulticastLock("edifierctrl").apply {
                setReferenceCounted(false)
                acquire()
            }
            check(bluetooth.getProfileProxy(
                application,
                object : BluetoothProfile.ServiceListener {
                    override fun onServiceConnected(profile: Int, proxy: BluetoothProfile) {
                        synchronized(this@BluetoothBridge) {
                            if (!started || generation != currentGeneration) {
                                runCatching { bluetooth.closeProfileProxy(profile, proxy) }
                                    .onFailure { Log.w(TAG, "释放过期蓝牙代理失败", it) }
                            } else if (profile == BluetoothProfile.A2DP) {
                                a2dp = proxy as BluetoothA2dp
                                Log.i(TAG, "A2DP 代理已就绪")
                            }
                        }
                    }

                    override fun onServiceDisconnected(profile: Int) {
                        synchronized(this@BluetoothBridge) {
                            if (generation == currentGeneration && profile == BluetoothProfile.A2DP) {
                                a2dp = null
                            }
                        }
                    }
                },
                BluetoothProfile.A2DP,
            )) { "无法取得系统蓝牙音频服务" }
            Log.i(TAG, "蓝牙桥已启动, Wi-Fi 组播锁已获取")
        } catch (error: Exception) {
            stop()
            throw error
        }
    }

    @JvmStatic
    @Synchronized
    fun stop() {
        started = false
        generation += 1
        closeRfcomm()
        val proxy = a2dp
        a2dp = null
        if (proxy != null) {
            runCatching { adapter()?.closeProfileProxy(BluetoothProfile.A2DP, proxy) }
                .onFailure { Log.w(TAG, "释放 A2DP 代理失败", it) }
        }
        val lock = multicastLock
        multicastLock = null
        runCatching { if (lock?.isHeld == true) lock.release() }
            .onFailure { Log.w(TAG, "释放 Wi-Fi 组播锁失败", it) }
        Log.i(TAG, "蓝牙桥已停止")
    }

    @JvmStatic
    fun scanRfcomm(): String {
        val adapter = adapter() ?: return "[]"
        val arr = JSONArray()
        for (device in adapter.bondedDevices.orEmpty()) {
            if (!isEdifier(device)) {
                continue
            }
            val o = JSONObject()
            o.put("address", device.address)
            o.put("name", device.name ?: "")
            o.put("kind", "rfcomm")
            o.put("service_uuid", RFCOMM)
            arr.put(o)
        }
        Log.i(TAG, "edifier=${arr.length()}")
        return arr.toString()
    }

    @JvmStatic
    fun connectedEdifier(): String {
        waitA2dp()
        val arr = JSONArray()
        val seen = HashSet<String>()
        fun add(device: BluetoothDevice) {
            if (!isEdifier(device)) {
                return
            }
            val addr = device.address ?: return
            if (!seen.add(addr)) {
                return
            }
            val o = JSONObject()
            o.put("address", addr)
            o.put("name", device.name ?: "")
            arr.put(o)
        }
        a2dp?.connectedDevices.orEmpty().forEach(::add)
        Log.i(TAG, "connectedEdifier=${arr.length()}")
        return arr.toString()
    }

    @JvmStatic
    fun openRfcomm(address: String): Int {
        return try {
            closeRfcomm()
            val adapter = adapter() ?: return fail("没有蓝牙适配器")
            adapter.cancelDiscovery()
            val device = adapter.getRemoteDevice(address)
            val sock = device.createRfcommSocketToServiceRecord(UUID.fromString(RFCOMM))
            sock.connect()
            socket = sock
            lastErrorText = ""
            Log.i(TAG, "RFCOMM 已连接 $address")
            0
        } catch (e: Exception) {
            fail(e.message ?: "RFCOMM 连接失败")
        }
    }

    @JvmStatic
    fun writeRfcomm(bytes: ByteArray): Int {
        return try {
            val out = socket?.outputStream ?: return fail("尚未打开 RFCOMM")
            out.write(bytes)
            out.flush()
            0
        } catch (e: Exception) {
            fail(e.message ?: "RFCOMM 写入失败")
        }
    }

    @JvmStatic
    fun readRfcomm(max: Int): ByteArray {
        val sock = socket ?: return ByteArray(0)
        return try {
            val input = sock.inputStream
            val avail = input.available()
            if (avail <= 0) {
                return ByteArray(0)
            }
            val buf = ByteArray(max.coerceAtLeast(1).coerceAtMost(avail))
            val n = input.read(buf)
            if (n <= 0) ByteArray(0) else buf.copyOf(n)
        } catch (_: java.net.SocketTimeoutException) {
            ByteArray(0)
        } catch (e: Exception) {
            lastErrorText = e.message ?: "RFCOMM 读取失败"
            ByteArray(0)
        }
    }

    @JvmStatic
    fun closeRfcomm(): Int {
        return try {
            socket?.close()
            socket = null
            0
        } catch (e: Exception) {
            socket = null
            fail(e.message ?: "RFCOMM 关闭失败")
        }
    }

    @JvmStatic
    fun audioState(address: String): String {
        val proxy = a2dp ?: return "unknown"
        return try {
            val device = adapter()?.getRemoteDevice(address) ?: return "unknown"
            when (proxy.getConnectionState(device)) {
                BluetoothProfile.STATE_CONNECTED -> "connected"
                BluetoothProfile.STATE_CONNECTING -> "connecting"
                BluetoothProfile.STATE_DISCONNECTED -> "disconnected"
                else -> "unknown"
            }
        } catch (error: Exception) {
            val message = "读取系统音频失败: ${error.message}"
            if (lastErrorText != message) Log.w(TAG, message, error)
            lastErrorText = message
            "unknown"
        }
    }

    @JvmStatic
    fun connectAudio(address: String): Int {
        waitA2dp()
        if (audioState(address) == "connected") return 0
        val request = invokeA2dp("connect", address)
        repeat(25) {
            if (audioState(address) == "connected") {
                return 0
            }
            Thread.sleep(100)
        }
        return if (request != 0) -1 else fail("未确认系统音频连接, 可在系统蓝牙设置中连接后重试")
    }

    @JvmStatic
    fun disconnectAudio(address: String): Int {
        if (audioState(address) == "disconnected") return 0
        val request = invokeA2dp("disconnect", address)
        repeat(5) {
            if (audioState(address) == "disconnected") {
                return 0
            }
            Thread.sleep(100)
        }
        return if (request != 0) -1 else fail("尚未确认系统音频断开")
    }

    private fun waitA2dp() {
        if (a2dp != null) {
            return
        }
        repeat(20) {
            if (a2dp != null) {
                return
            }
            Thread.sleep(100)
        }
    }

    private fun invokeA2dp(method: String, address: String): Int {
        return try {
            val proxy = a2dp ?: return fail("A2DP 代理未就绪")
            val device = adapter()?.getRemoteDevice(address) ?: return fail("找不到设备")
            val m = BluetoothA2dp::class.java.getMethod(method, BluetoothDevice::class.java)
            m.isAccessible = true
            when (val r = m.invoke(proxy, device)) {
                is Boolean -> if (r) 0 else fail("A2DP $method 返回 false")
                else -> 0
            }
        } catch (e: Exception) {
            fail("A2DP $method: ${e.message}")
        }
    }

    private fun isEdifier(device: BluetoothDevice): Boolean {
        val name = device.name.orEmpty()
        if (name.contains("EDIFIER", ignoreCase = true) || name.contains("漫步者")) {
            return true
        }
        val target = UUID.fromString(RFCOMM)
        return device.uuids.orEmpty().any { it.uuid == target }
    }

    private fun adapter(): BluetoothAdapter? = BluetoothAdapter.getDefaultAdapter()

    private fun fail(msg: String): Int {
        lastErrorText = msg
        Log.e(TAG, msg)
        return -1
    }
}
