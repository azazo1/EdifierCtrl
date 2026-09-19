package dev.edifierctrl.app.infrastructure

import android.content.SharedPreferences
import java.util.UUID

internal data class PreferencesDocument(
    val schemaVersion: Int = PreferencesMigration.CURRENT_VERSION,
    val rememberGroup: Boolean = true,
    val autoJoinGroup: Boolean = false,
    val groupName: String = "",
    val selectedProfile: String = "basedevice",
    val lastDeviceAddress: String = "",
    val lastDeviceName: String = "",
    val installationId: String = UUID.randomUUID().toString(),
    val theme: String = "system",
)

/** 版本 0 为旧 passphrase 格式, 版本 1 明确存储组名和启动偏好. */
internal object PreferencesMigration {
    const val CURRENT_VERSION = 1

    fun read(preferences: SharedPreferences): PreferencesDocument {
        val version = preferences.getInt("schemaVersion", 0)
        return when (version) {
            0 -> migrateVersionZero(preferences)
            CURRENT_VERSION -> readVersionOne(preferences)
            else -> error("不支持设置版本 $version, 当前支持版本 $CURRENT_VERSION. 原设置已保留.")
        }
    }

    private fun migrateVersionZero(preferences: SharedPreferences): PreferencesDocument {
        val group = preferences.getString("passphrase", "").orEmpty()
        return PreferencesDocument(
            schemaVersion = 1,
            rememberGroup = group.isNotBlank(),
            autoJoinGroup = false,
            groupName = group,
        )
    }

    private fun readVersionOne(preferences: SharedPreferences): PreferencesDocument {
        val remember = preferences.getBoolean("rememberGroup", true)
        val installation = preferences.getString("installationId", "").orEmpty()
        val validInstallation = runCatching { UUID.fromString(installation).toString() }.getOrNull()
        val theme = preferences.getString("theme", "system").orEmpty()
        return PreferencesDocument(
            rememberGroup = remember,
            autoJoinGroup = remember && preferences.getBoolean("autoJoinGroup", false),
            groupName = if (remember) preferences.getString("groupName", "").orEmpty() else "",
            selectedProfile = preferences.getString("selectedProfile", "basedevice").orEmpty().ifBlank { "basedevice" },
            lastDeviceAddress = preferences.getString("lastDeviceAddress", "").orEmpty(),
            lastDeviceName = preferences.getString("lastDeviceName", "").orEmpty(),
            installationId = validInstallation ?: UUID.randomUUID().toString(),
            theme = theme.takeIf { it in setOf("system", "light", "dark") } ?: "system",
        )
    }

    fun write(preferences: SharedPreferences, value: PreferencesDocument) {
        val saved = preferences.edit()
            .putInt("schemaVersion", value.schemaVersion)
            .putBoolean("rememberGroup", value.rememberGroup)
            .putBoolean("autoJoinGroup", value.rememberGroup && value.autoJoinGroup)
            .putString("groupName", if (value.rememberGroup) value.groupName else "")
            .putString("selectedProfile", value.selectedProfile)
            .putString("lastDeviceAddress", value.lastDeviceAddress)
            .putString("lastDeviceName", value.lastDeviceName)
            .putString("installationId", value.installationId)
            .putString("theme", value.theme)
            .remove("passphrase")
            .commit()
        check(saved) { "设置保存失败, 请检查设备存储空间" }
    }
}
