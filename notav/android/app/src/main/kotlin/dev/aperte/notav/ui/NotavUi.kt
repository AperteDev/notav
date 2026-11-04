package dev.aperte.notav.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.outlined.Notes
import androidx.compose.material.icons.outlined.ErrorOutline
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.produceState
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import dev.aperte.notav.NotavApp
import dev.aperte.notav.core.Account
import dev.aperte.notav.core.AccountKind
import dev.aperte.notav.core.Note
import dev.aperte.notav.core.NotavException
import dev.aperte.notav.tr
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/** What the app shows: the welcome page without an account, else the notes. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun NotavUi(app: NotavApp) {
    val changes by app.changes.collectAsStateWithLifecycle()
    val data by produceState<Pair<List<Account>, List<Note>>?>(null, changes) {
        value = withContext(Dispatchers.IO) { app.core.accounts() to app.core.notes() }
    }
    val (accounts, notes) = data ?: return
    if (accounts.isEmpty()) {
        Welcome(app)
        return
    }
    Scaffold(topBar = { TopAppBar(title = { Text("Notav") }) }) { padding ->
        LazyColumn(Modifier.padding(padding)) {
            items(notes, key = { it.id }) { note ->
                ListItem(
                    onClick = {},
                    supportingContent = note.folder.takeIf { it.isNotEmpty() }?.let { { Text(it) } },
                    leadingContent = { Icon(Icons.AutoMirrored.Outlined.Notes, null) },
                ) { Text(note.title) }
            }
        }
    }
}

/** No account yet: keep notes on this device. */
@Composable
private fun Welcome(app: NotavApp) {
    val scope = rememberCoroutineScope()
    Page(
        title = "Notav",
        text = tr("Notes and a journal kept as Markdown files in your Nextcloud, or on this device"),
    ) {
        Button(onClick = {
            scope.launch(Dispatchers.IO) {
                val id = app.core.addAccount(AccountKind.LOCAL, "", "", "")
                runCatching {
                    app.core.addNote(
                        id,
                        "",
                        tr("Welcome to Notav"),
                        tr(
                            "Your notes stay on this device. They are written in Markdown: # starts a " +
                                "heading, - a list, - [ ] a task, and [[Title]] links another note.",
                        ),
                    )
                }
                app.changed()
            }
        }) { Text(tr("On This Device Only")) }
    }
}

/** The data could not be opened (left by a newer Notav, say): only that, nothing else runs. */
@Composable
fun OpenFailed(e: Throwable) {
    Page(
        title = tr("Notav Cannot Open Its Data"),
        text = when (e) {
            is NotavException.Newer -> tr(
                "It was saved by a newer version of Notav. Update Notav to open it; this version " +
                    "leaves it as it is.",
            )
            else -> e.message ?: e.toString()
        },
        icon = { Icon(Icons.Outlined.ErrorOutline, null) },
    )
}

/** A page with a title and a text in the middle, and what goes under them. */
@Composable
private fun Page(
    title: String,
    text: String,
    icon: @Composable () -> Unit = {},
    actions: @Composable () -> Unit = {},
) {
    Scaffold { padding ->
        Column(
            Modifier.fillMaxSize().padding(padding).padding(32.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp, Alignment.CenterVertically),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            icon()
            Text(title, style = MaterialTheme.typography.headlineMedium, textAlign = TextAlign.Center)
            Text(text, style = MaterialTheme.typography.bodyLarge, textAlign = TextAlign.Center)
            actions()
        }
    }
}
