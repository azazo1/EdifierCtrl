package dev.edifierctrl.app

import android.util.Log

/** JNI 声明只由应用会话 worker 调用, 不持有 Activity 或会话句柄. */
object EdifierNative {
    var loaded: Boolean = false
        private set
    var loadError: String? = null
        private set

    fun ensureLoaded(): Boolean {
        if (loaded) return true
        return try {
            System.loadLibrary("edifier_ffi")
            loaded = true
            loadError = null
            true
        } catch (error: LinkageError) {
            loadError = error.message ?: "无法加载 libedifier_ffi.so"
            Log.e("EdifierNative", "加载核心失败", error)
            false
        } catch (error: SecurityException) {
            loadError = error.message ?: "系统拒绝加载核心"
            Log.e("EdifierNative", "加载核心失败", error)
            false
        }
    }

    external fun version(): String
    external fun lastError(): String?
    external fun commandEncode(commandJson: String): String?
    external fun frameParse(frameHex: String): String?
    external fun profilesJson(): String?
    external fun sessionNew(localId: String): Long
    external fun sessionFree(session: Long)
    external fun sessionScan(session: Long, kind: String): String?
    external fun sessionConnect(session: Long, address: String, kind: String): Int
    external fun sessionDisconnect(session: Long): Int
    external fun sessionReadout(session: Long, profileKey: String): Int
    external fun sessionSendJson(session: Long, commandJson: String): Int
    external fun sessionPollEvent(session: Long): String?
    external fun sessionGroupJoin(session: Long, passphrase: String): Int
    external fun sessionGroupLeave(session: Long): Int
    external fun sessionGroupPeers(session: Long): String?
    external fun sessionGroupClaim(session: Long, mac: String): Int
    external fun sessionGroupClaimPeer(session: Long, peerId: String): Int
    external fun sessionGroupIdHex(session: Long): String?
    external fun sessionSetHolding(session: Long, mac: String): Int
    external fun sessionHolding(session: Long): String?
}
