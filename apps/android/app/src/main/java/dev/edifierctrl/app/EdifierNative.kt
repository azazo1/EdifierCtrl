package dev.edifierctrl.app

/**
 * JNI 对应 crates/edifier-ffi/include/edifier.h.
 * 实现随 libedifier_ffi.so 接入.
 */
object EdifierNative {
    var loaded: Boolean = true
        private set

    init {
        try {
            System.loadLibrary("edifier_ffi")
        } catch (_: UnsatisfiedLinkError) {
            loaded = false
        }
    }

    @Volatile
    private var sessionHandle: Long = 0

    @Synchronized
    fun ensureSession(): Long {
        if (!loaded) {
            return 0
        }
        if (sessionHandle == 0L) {
            sessionHandle = sessionNew(android.os.Build.MODEL)
        }
        return sessionHandle
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
    external fun sessionGroupPeers(session: Long): String?
    external fun sessionGroupClaim(session: Long, mac: String): Int
    external fun sessionGroupClaimPeer(session: Long, peerId: String): Int
    external fun sessionGroupIdHex(session: Long): String?
    external fun sessionSetHolding(session: Long, mac: String): Int
    external fun sessionHolding(session: Long): String?
}
