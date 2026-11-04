package dev.aperte.notav

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.viewModels
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.lifecycleScope
import androidx.lifecycle.repeatOnLifecycle
import dev.aperte.notav.ui.NotavTheme
import dev.aperte.notav.ui.NotavUi
import dev.aperte.notav.ui.OpenFailed
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

class MainActivity : ComponentActivity() {
    private val model: Main by viewModels()

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
            NotavTheme(scheme) { NotavUi(model) }
        }
        if (savedInstanceState == null) handle(intent)
        // In front: sync now, then every 30 s for changes from other devices (each notes
        // folder's ETag: one request when nothing changed), as the Linux app while focused.
        lifecycleScope.launch {
            repeatOnLifecycle(Lifecycle.State.RESUMED) {
                model.inFront = true
                try {
                    app.awaitReady()
                    while (true) {
                        app.syncNow()
                        delay(30_000)
                    }
                } finally {
                    model.inFront = false
                }
            }
        }
    }

    override fun onNewIntent(intent: android.content.Intent) {
        super.onNewIntent(intent)
        if ((application as NotavApp).failed == null) handle(intent)
    }

    /** Text shared from another app: a new note. */
    private fun handle(intent: android.content.Intent?) {
        if (intent?.action != android.content.Intent.ACTION_SEND) return
        val text = intent.getStringExtra(android.content.Intent.EXTRA_TEXT) ?: return
        model.shared(intent.getStringExtra(android.content.Intent.EXTRA_SUBJECT), text)
    }

    /** Left: what waits for the server goes up once there is a network. */
    override fun onStop() {
        super.onStop()
        val app = application as NotavApp
        if (app.failed == null && !isChangingConfigurations) Upload.schedule(app)
    }
}
