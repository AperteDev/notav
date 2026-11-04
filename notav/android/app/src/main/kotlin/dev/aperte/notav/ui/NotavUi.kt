package dev.aperte.notav.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.ErrorOutline
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import dev.aperte.notav.Main
import dev.aperte.notav.core.NotavException
import dev.aperte.notav.reason
import dev.aperte.notav.tr

/** What the app shows: the welcome steps until an account has its notes, then the notes. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun NotavUi(model: Main) {
    val screen by model.screen.collectAsStateWithLifecycle()
    val firstSync by model.firstSync.collectAsStateWithLifecycle()
    val s = screen ?: return
    firstSync?.let {
        FirstSync(model, it)
        return
    }
    if (s.accounts.isEmpty()) {
        Welcome(model)
        return
    }
    s.needsFolder?.let {
        NotesFolder(model, it)
        return
    }
    val page by model.page.collectAsStateWithLifecycle()
    WithMessages(model) {
        page?.let { PageView(model, it) } ?: Home(model, s)
    }
}

/** The data could not be opened (left by a newer Notav, say): only that, nothing else runs. */
@Composable
fun OpenFailed(e: Throwable) {
    Scaffold { padding ->
        Column(
            Modifier.fillMaxSize().padding(padding).padding(32.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp, Alignment.CenterVertically),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            Icon(Icons.Outlined.ErrorOutline, null)
            Text(
                tr("Notav Cannot Open Its Data"),
                style = MaterialTheme.typography.headlineMedium,
                textAlign = TextAlign.Center,
            )
            Text(
                if (e is NotavException) reason(e) else e.message ?: e.toString(),
                style = MaterialTheme.typography.bodyLarge,
                textAlign = TextAlign.Center,
            )
        }
    }
}
