package dev.edifierctrl.app.infrastructure

import android.content.Context
import android.content.SharedPreferences
import android.os.Handler
import android.os.Looper
import android.util.Log
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import java.util.concurrent.Executors

/** 设置读写使用独立串行队列, Compose 只在主线程接收快照. */
object AppPreferences {
    private const val TAG = "EdifierPreferences"
    private val main = Handler(Looper.getMainLooper())
    private val storage = Executors.newSingleThreadExecutor { task -> Thread(task, "edifier-preferences") }
    private var preferences: SharedPreferences? = null
    private var initialized = false
    private var loaded = false
    private var writable = false
    private var revision = 0L
    private val waiting = mutableListOf<() -> Unit>()
    private var document by mutableStateOf(PreferencesDocument())

    val rememberGroup: Boolean get() = document.rememberGroup
    val autoJoinGroup: Boolean get() = document.autoJoinGroup
    val groupName: String get() = document.groupName
    val selectedProfile: String get() = document.selectedProfile
    val lastDeviceAddress: String get() = document.lastDeviceAddress
    val lastDeviceName: String get() = document.lastDeviceName
    val installationId: String get() = document.installationId
    val theme: String get() = document.theme
    var saveError by mutableStateOf<String?>(null)
        private set

    fun initialize(context: Context) {
        val application = context.applicationContext
        onMain {
            if (initialized) return@onMain
            initialized = true
            storage.execute {
                var decoded: PreferencesDocument? = null
                var failure: Throwable? = null
                try {
                    val store = application.getSharedPreferences("edifierctrl", Context.MODE_PRIVATE)
                    preferences = store
                    decoded = PreferencesMigration.read(store)
                    // 完整解码后才允许保存, 较新版本或损坏文档不被默认值覆盖.
                    writable = true
                    PreferencesMigration.write(store, decoded)
                } catch (error: Throwable) {
                    failure = error
                    Log.e(TAG, "读取或迁移设置失败", error)
                }
                val snapshot = decoded
                val errorText = failure?.message
                onMain {
                    if (snapshot != null) document = snapshot
                    saveError = errorText
                    loaded = true
                    val callbacks = waiting.toList()
                    waiting.clear()
                    callbacks.forEach { callback -> callback() }
                }
            }
        }
    }

    fun setRememberGroup(value: Boolean) = mutate { current ->
        current.copy(
            rememberGroup = value,
            groupName = if (value) current.groupName else "",
            autoJoinGroup = value && current.autoJoinGroup,
        )
    }

    fun setAutoJoinGroup(value: Boolean) = mutate { current ->
        current.copy(autoJoinGroup = value && current.rememberGroup)
    }

    fun setTheme(value: String) {
        if (value !in setOf("system", "light", "dark")) {
            onMain { saveError = "主题必须为 system, light 或 dark" }
            return
        }
        mutate { it.copy(theme = value) }
    }

    internal fun whenReady(action: () -> Unit) = onMain {
        if (loaded) action() else waiting.add(action)
    }

    internal fun rememberJoinedGroup(group: String, remember: Boolean) = mutate { current ->
        current.copy(
            rememberGroup = remember,
            groupName = if (remember) group else "",
            autoJoinGroup = remember && current.autoJoinGroup,
        )
    }

    internal fun rememberDevice(address: String, name: String, profileId: String) = mutate { current ->
        current.copy(lastDeviceAddress = address, lastDeviceName = name, selectedProfile = profileId)
    }

    internal fun selectProfile(id: String) = mutate { it.copy(selectedProfile = id) }

    private fun mutate(change: (PreferencesDocument) -> PreferencesDocument) = onMain {
        whenReady {
            document = change(document)
            val snapshot = document
            val savingRevision = ++revision
            storage.execute {
                if (!writable) return@execute
                val error = runCatching {
                    val store = checkNotNull(preferences) { "设置尚未初始化" }
                    PreferencesMigration.write(store, snapshot)
                }.exceptionOrNull()
                if (error != null) Log.e(TAG, "保存设置失败", error)
                onMain {
                    if (savingRevision == revision) saveError = error?.message
                }
            }
        }
    }

    private fun onMain(action: () -> Unit) {
        if (Looper.myLooper() == Looper.getMainLooper()) action() else main.post { action() }
    }
}
