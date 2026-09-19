package dev.edifierctrl.app.ui

import android.app.Activity
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.SideEffect
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalView
import androidx.core.view.WindowCompat
import dev.edifierctrl.app.infrastructure.AppPreferences

private val Dark = darkColorScheme(
    primary = Color(0xFFAFC6FF),
    onPrimary = Color(0xFF082C70),
    primaryContainer = Color(0xFF253F72),
    onPrimaryContainer = Color(0xFFDAE5FF),
    secondary = Color(0xFFA7D9C4),
    onSecondary = Color(0xFF10382B),
    background = Color(0xFF111318),
    surface = Color(0xFF181B22),
    surfaceVariant = Color(0xFF2A303C),
    onBackground = Color(0xFFE3E7F0),
    onSurface = Color(0xFFE3E7F0),
    onSurfaceVariant = Color(0xFFBAC4D5),
    error = Color(0xFFFFB4AB),
)

private val Light = lightColorScheme(
    primary = Color(0xFF3562D8),
    onPrimary = Color.White,
    primaryContainer = Color(0xFFE2EBFF),
    onPrimaryContainer = Color(0xFF183365),
    secondary = Color(0xFF326B56),
    onSecondary = Color.White,
    background = Color(0xFFF6F8FC),
    surface = Color(0xFFFCFDFF),
    surfaceVariant = Color(0xFFEAF0F9),
    onBackground = Color(0xFF1B2537),
    onSurface = Color(0xFF1B2537),
    onSurfaceVariant = Color(0xFF53627A),
    error = Color(0xFFBA1A1A),
)

@Composable
fun EdifierTheme(content: @Composable () -> Unit) {
    val dark = when (AppPreferences.theme) {
        "light" -> false
        "dark" -> true
        else -> isSystemInDarkTheme()
    }
    val view = LocalView.current
    SideEffect {
        val activity = view.context as? Activity
        if (activity != null && !view.isInEditMode) {
            WindowCompat.getInsetsController(activity.window, view).apply {
                isAppearanceLightStatusBars = !dark
                isAppearanceLightNavigationBars = !dark
            }
        }
    }
    MaterialTheme(colorScheme = if (dark) Dark else Light, content = content)
}
