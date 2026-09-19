package dev.edifierctrl.app

import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import android.graphics.Color
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.provider.Settings
import android.util.Log
import android.widget.Toast
import androidx.activity.ComponentActivity
import androidx.activity.SystemBarStyle
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import dev.edifierctrl.app.session.AppActions
import dev.edifierctrl.app.ui.EdifierTheme
import dev.edifierctrl.app.ui.WorkspaceApp

class MainActivity : ComponentActivity() {
    private val bluetoothPermissionRequest = registerForActivityResult(ActivityResultContracts.RequestMultiplePermissions()) {
        refreshPermission()
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge(
            statusBarStyle = SystemBarStyle.auto(Color.TRANSPARENT, Color.TRANSPARENT),
            navigationBarStyle = SystemBarStyle.auto(Color.TRANSPARENT, Color.TRANSPARENT),
        )
        setContent {
            EdifierTheme {
                WorkspaceApp(
                    onRequestPermission = { bluetoothPermissionRequest.launch(bluetoothPermissions()) },
                    onBluetoothSettings = { openSettings(Intent(Settings.ACTION_BLUETOOTH_SETTINGS)) },
                    onAppSettings = {
                        openSettings(Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS, Uri.parse("package:$packageName")))
                    },
                )
            }
        }
    }

    override fun onResume() {
        super.onResume()
        // 从系统设置返回时也重新确认权限, 不在重组或旋转时创建第二份会话.
        refreshPermission()
    }

    private fun refreshPermission() {
        val granted = bluetoothPermissions().all { checkSelfPermission(it) == PackageManager.PERMISSION_GRANTED }
        AppActions.updateBluetoothPermission(granted)
    }

    private fun bluetoothPermissions(): Array<String> = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
        arrayOf(Manifest.permission.BLUETOOTH_CONNECT, Manifest.permission.BLUETOOTH_SCAN)
    } else {
        arrayOf(Manifest.permission.ACCESS_FINE_LOCATION)
    }

    private fun openSettings(intent: Intent) {
        try {
            startActivity(intent)
        } catch (error: RuntimeException) {
            Log.w("EdifierUi", "无法打开系统设置", error)
            Toast.makeText(this, "无法打开此设置页, 请从手机设置中进入蓝牙或应用权限.", Toast.LENGTH_LONG).show()
        }
    }
}
