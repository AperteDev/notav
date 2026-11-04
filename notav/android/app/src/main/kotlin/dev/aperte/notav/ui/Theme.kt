package dev.aperte.notav.ui

import android.os.Build
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.MaterialExpressiveTheme
import androidx.compose.material3.MotionScheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.dynamicDarkColorScheme
import androidx.compose.material3.dynamicLightColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext

/**
 * Light or dark as `scheme` says ("light", "dark", else the system's); the wallpaper's colours
 * where Android has them (12 and later), else the icon's yellows. Todav's TodavTheme.
 */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun NotavTheme(scheme: String?, content: @Composable () -> Unit) {
    val dark = when (scheme) {
        "light" -> false
        "dark" -> true
        else -> isSystemInDarkTheme()
    }
    val context = LocalContext.current
    val colors = when {
        Build.VERSION.SDK_INT >= Build.VERSION_CODES.S ->
            if (dark) dynamicDarkColorScheme(context) else dynamicLightColorScheme(context)
        // The icon's header, where the system gives no colours of its own.
        dark -> darkColorScheme(primary = Color(0xFFF5C84C), secondary = Color(0xFF8EC4EE))
        else -> lightColorScheme(primary = Color(0xFF8A5A00), secondary = Color(0xFF1A5FB4))
    }
    MaterialExpressiveTheme(colorScheme = colors, motionScheme = MotionScheme.expressive(), content = content)
}
