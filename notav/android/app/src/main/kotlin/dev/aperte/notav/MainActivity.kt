package dev.aperte.notav

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import dev.aperte.notav.ui.NotavTheme
import dev.aperte.notav.ui.NotavUi
import dev.aperte.notav.ui.OpenFailed

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        // The activity has the app's own language when one is set in the system settings.
        I18n.load(this)
        enableEdgeToEdge()
        val app = application as NotavApp
        app.failed?.let { e ->
            setContent { NotavTheme(null) { OpenFailed(e) } }
            return
        }
        setContent {
            val changes by app.changes.collectAsStateWithLifecycle()
            // Light, dark or the system's, as the device-only `color_scheme` says.
            val scheme = remember(changes) { app.core.setting("color_scheme") }
            NotavTheme(scheme) { NotavUi(app) }
        }
    }
}
