package dev.edifierctrl.app

import android.Manifest
import android.os.Build
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.ui.Modifier
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.currentBackStackEntryAsState
import androidx.navigation.compose.rememberNavController
import dev.edifierctrl.app.ui.ControlScreen
import dev.edifierctrl.app.ui.DebugScreen
import dev.edifierctrl.app.ui.DeviceScreen
import dev.edifierctrl.app.ui.GroupScreen

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            requestPermissions(
                arrayOf(
                    Manifest.permission.BLUETOOTH_CONNECT,
                    Manifest.permission.BLUETOOTH_SCAN,
                    Manifest.permission.ACCESS_FINE_LOCATION,
                ),
                1,
            )
        }
        setContent {
            val nav = rememberNavController()
            val tabs = listOf("device" to "设备", "control" to "控制", "group" to "组", "debug" to "调试")
            Scaffold(
                bottomBar = {
                    val route = nav.currentBackStackEntryAsState().value?.destination?.route
                    NavigationBar {
                        tabs.forEach { (id, label) ->
                            NavigationBarItem(
                                selected = route == id,
                                onClick = { nav.navigate(id) },
                                icon = { Text(label.take(1)) },
                                label = { Text(label) },
                            )
                        }
                    }
                },
            ) { inner ->
                NavHost(navController = nav, startDestination = "device", modifier = Modifier.padding(inner)) {
                    composable("device") { DeviceScreen() }
                    composable("control") { ControlScreen() }
                    composable("group") { GroupScreen() }
                    composable("debug") { DebugScreen() }
                }
            }
        }
    }
}
