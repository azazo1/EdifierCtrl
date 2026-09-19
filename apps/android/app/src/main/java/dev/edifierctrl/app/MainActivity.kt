package dev.edifierctrl.app

import android.Manifest
import android.graphics.Color
import android.os.Build
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.SystemBarStyle
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.WindowInsetsSides
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.only
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.BugReport
import androidx.compose.material.icons.outlined.Groups
import androidx.compose.material.icons.outlined.Headphones
import androidx.compose.material.icons.outlined.Tune
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarDefaults
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.ui.Modifier
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.currentBackStackEntryAsState
import androidx.navigation.compose.rememberNavController
import dev.edifierctrl.app.ui.ControlScreen
import dev.edifierctrl.app.ui.DebugScreen
import dev.edifierctrl.app.ui.DeviceScreen
import dev.edifierctrl.app.ui.EdifierTheme
import dev.edifierctrl.app.ui.EventPump
import dev.edifierctrl.app.ui.GroupScreen

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge(
            statusBarStyle = SystemBarStyle.auto(Color.TRANSPARENT, Color.TRANSPARENT),
            navigationBarStyle = SystemBarStyle.auto(Color.TRANSPARENT, Color.TRANSPARENT),
        )
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
            EdifierTheme {
                Surface(
                    modifier = Modifier.fillMaxSize(),
                    color = MaterialTheme.colorScheme.background,
                ) {
                    val nav = rememberNavController()
                    val tabs = listOf(
                        Triple("device", "设备", Icons.Outlined.Headphones),
                        Triple("control", "控制", Icons.Outlined.Tune),
                        Triple("group", "组", Icons.Outlined.Groups),
                        Triple("debug", "调试", Icons.Outlined.BugReport),
                    )
                    EventPump()
                    Scaffold(
                        modifier = Modifier.fillMaxSize(),
                        containerColor = MaterialTheme.colorScheme.background,
                        contentWindowInsets = WindowInsets.safeDrawing.only(
                            WindowInsetsSides.Horizontal + WindowInsetsSides.Top,
                        ),
                        bottomBar = {
                            val route = nav.currentBackStackEntryAsState().value?.destination?.route
                            NavigationBar(windowInsets = NavigationBarDefaults.windowInsets) {
                                tabs.forEach { (id, label, icon) ->
                                    NavigationBarItem(
                                        selected = route == id,
                                        onClick = {
                                            nav.navigate(id) {
                                                popUpTo("device") { saveState = true }
                                                launchSingleTop = true
                                                restoreState = true
                                            }
                                        },
                                        icon = { Icon(icon, contentDescription = label) },
                                        label = { Text(label) },
                                    )
                                }
                            }
                        },
                    ) { inner ->
                        NavHost(
                            navController = nav,
                            startDestination = "device",
                            modifier = Modifier.fillMaxSize().padding(inner),
                        ) {
                            composable("device") { DeviceScreen() }
                            composable("control") { ControlScreen() }
                            composable("group") { GroupScreen() }
                            composable("debug") { DebugScreen() }
                        }
                    }
                }
            }
        }
    }
}
