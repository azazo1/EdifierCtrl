package dev.edifierctrl.app.ui

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color

private val Dark = darkColorScheme(
    primary = Color(0xFFE08A4C),
    onPrimary = Color(0xFF3A1C07),
    primaryContainer = Color(0xFF5C3110),
    onPrimaryContainer = Color(0xFFFFDCC4),
    secondary = Color(0xFF8FCBB5),
    onSecondary = Color(0xFF0B3328),
    background = Color(0xFF141210),
    surface = Color(0xFF1C1A18),
    surfaceVariant = Color(0xFF2C2824),
    onBackground = Color(0xFFEDE0D4),
    onSurface = Color(0xFFEDE0D4),
    onSurfaceVariant = Color(0xFFD0C4B8),
    error = Color(0xFFFFB4AB),
)

private val Light = lightColorScheme(
    primary = Color(0xFF9A4A16),
    onPrimary = Color(0xFFFFFFFF),
    primaryContainer = Color(0xFFFFDCC4),
    onPrimaryContainer = Color(0xFF341000),
    secondary = Color(0xFF3B6658),
    onSecondary = Color(0xFFFFFFFF),
    background = Color(0xFFFFF8F4),
    surface = Color(0xFFFFF8F4),
    surfaceVariant = Color(0xFFF4E6DC),
    onBackground = Color(0xFF211A14),
    onSurface = Color(0xFF211A14),
    onSurfaceVariant = Color(0xFF52443B),
    error = Color(0xFFBA1A1A),
)

@Composable
fun EdifierTheme(content: @Composable () -> Unit) {
    MaterialTheme(
        colorScheme = if (isSystemInDarkTheme()) Dark else Light,
        content = content,
    )
}
