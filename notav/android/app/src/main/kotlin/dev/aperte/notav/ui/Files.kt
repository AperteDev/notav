package dev.aperte.notav.ui

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowRight
import androidx.compose.material.icons.automirrored.outlined.Notes
import androidx.compose.material.icons.automirrored.outlined.Undo
import androidx.compose.material.icons.outlined.CloudOff
import androidx.compose.material.icons.outlined.DeleteForever
import androidx.compose.material.icons.outlined.DeleteOutline
import androidx.compose.material.icons.outlined.Folder
import androidx.compose.material.icons.outlined.History
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LoadingIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import dev.aperte.notav.Main
import dev.aperte.notav.Message
import dev.aperte.notav.accountName
import dev.aperte.notav.core.Deleted
import dev.aperte.notav.core.Version
import dev.aperte.notav.reason
import dev.aperte.notav.titleOf
import dev.aperte.notav.tr
import java.text.DateFormat
import java.util.Date
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

// What Nextcloud keeps besides the notes, as the Linux app's files.rs: the Trash and a note's
// Versions, each asked of the server as the page opens (a loading indicator meanwhile).

/** A date and time in the locale's format. */
private fun whenText(seconds: Long): String =
    DateFormat.getDateTimeInstance(DateFormat.SHORT, DateFormat.SHORT).format(Date(seconds * 1000))

/** What the server was asked for: loading, failed (why), or got. */
private sealed interface Asked<out T> {
    data object Loading : Asked<Nothing>
    data class Failed(val why: String) : Asked<Nothing>
    data class Got<T>(val value: T) : Asked<T>
}

/** A page's state that is not a list: loading, failed or empty, centred. */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun Status(asked: Asked<*>, icon: ImageVector, failedTitle: String, emptyTitle: String, emptyText: String) = Box(
    Modifier.fillMaxWidth().heightIn(min = 400.dp).padding(32.dp),
    Alignment.Center,
) {
    when (asked) {
        Asked.Loading -> LoadingIndicator(Modifier.size(48.dp))
        else -> Column(horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(12.dp)) {
            Icon(if (asked is Asked.Failed) Icons.Outlined.CloudOff else icon, null, Modifier.size(48.dp))
            Text(
                if (asked is Asked.Failed) failedTitle else emptyTitle,
                style = MaterialTheme.typography.titleLarge,
                textAlign = TextAlign.Center,
            )
            Text(
                if (asked is Asked.Failed) asked.why else emptyText,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                textAlign = TextAlign.Center,
            )
        }
    }
}

/**
 * The Trash: what was deleted from the notes folders of the accounts whose server keeps it,
 * latest first, each to restore or (where the server lets it) to delete for good. What waits
 * for the server goes first, so notes deleted here are listed too.
 */
@Composable
fun TrashPage(model: Main, back: () -> Unit) = Frame(tr("Trash"), back) {
    val core = model.core
    var asked by remember { mutableStateOf<Asked<Unit>>(Asked.Loading) }
    // (account, its name with more than one, what was deleted)
    val rows = remember { mutableStateListOf<Triple<Long, String?, Deleted>>() }
    LaunchedEffect(Unit) {
        asked = withContext(Dispatchers.IO) {
            try {
                runCatching { core.sync() }
                val accounts = model.trashAccounts()
                val all = accounts.flatMap { a ->
                    core.trash(a.id).map { Triple(a.id, accountName(a).takeIf { accounts.size > 1 }, it) }
                }
                rows.addAll(all.sortedByDescending { it.third.deleted })
                Asked.Got(Unit)
            } catch (e: Exception) {
                Asked.Failed(reason(e))
            }
        }
    }
    var purging by remember { mutableStateOf<Triple<Long, String?, Deleted>?>(null) }
    val scope = rememberCoroutineScope()
    if (asked !is Asked.Got || rows.isEmpty()) {
        Status(
            asked, Icons.Outlined.DeleteOutline, tr("Cannot Open the Trash"), tr("Trash Is Empty"),
            tr("Deleted notes are kept here for a while"),
        )
        return@Frame
    }
    Note(tr("Your Nextcloud keeps deleted notes for a while, as its administrator set it, then deletes them for good."))
    Section(null) {
        for (row in rows.toList()) {
            val (account, from, d) = row
            val (folder, name) = d.path.substringBeforeLast('/', "") to d.path.substringAfterLast('/')
            val title = if (d.folder) name else titleOf(name)
            val purge = remember(account) { core.features(account).purge }
            Item(
                headlineContent = { Text(title) },
                supportingContent = {
                    Text(listOfNotNull(from, folder.takeIf { it.isNotEmpty() }, tr("Deleted {date}").replace("{date}", whenText(d.deleted))).joinToString(" · "))
                },
                leadingContent = { Icon(if (d.folder) Icons.Outlined.Folder else Icons.AutoMirrored.Outlined.Notes, null) },
                trailingContent = {
                    androidx.compose.foundation.layout.Row {
                        Tip(tr("Restore")) {
                            IconButton(onClick = {
                                scope.launch {
                                    try {
                                        withContext(Dispatchers.IO) { core.restoreDeleted(account, d.id) }
                                        rows.remove(row)
                                        model.notav.syncNow()
                                        model.messages.tryEmit(Message(tr("“{title}” restored").replace("{title}", title)))
                                    } catch (e: Exception) {
                                        model.messages.tryEmit(Message(reason(e)))
                                    }
                                }
                            }) { Icon(Icons.AutoMirrored.Outlined.Undo, tr("Restore")) }
                        }
                        if (purge) {
                            Tip(tr("Delete Permanently")) {
                                IconButton(onClick = { purging = row }) { Icon(Icons.Outlined.DeleteForever, tr("Delete Permanently")) }
                            }
                        }
                    }
                },
            )
        }
    }
    purging?.let { row ->
        val (account, _, d) = row
        val title = d.path.substringAfterLast('/').let { if (d.folder) it else titleOf(it) }
        AlertDialog(
            onDismissRequest = { purging = null },
            title = { Text(tr("Delete Permanently?")) },
            text = { Text(tr("“{title}” cannot be restored afterwards").replace("{title}", title)) },
            confirmButton = {
                TextButton(onClick = {
                    purging = null
                    scope.launch {
                        try {
                            withContext(Dispatchers.IO) { core.purge(account, d.id) }
                            rows.remove(row)
                        } catch (e: Exception) {
                            model.messages.tryEmit(Message(reason(e)))
                        }
                    }
                }) { Text(tr("Delete"), color = MaterialTheme.colorScheme.error) }
            },
            dismissButton = { TextButton(onClick = { purging = null }) { Text(tr("Cancel")) } },
        )
    }
}

/**
 * Note `id`'s versions that Nextcloud keeps, latest first; one opened shows its text, and
 * Restore makes that the note's. What waits for the server goes first, so the latest version
 * is the note as it is.
 */
@Composable
fun VersionsPage(model: Main, id: Long, back: () -> Unit) {
    val core = model.core
    val note = remember(id) { core.note(id) }
    // The version shown: (it, its text).
    var shown by remember { mutableStateOf<Pair<Version, String>?>(null) }
    shown?.let { (v, text) ->
        BackHandler { shown = null }
        Frame(
            whenText(v.modified),
            { shown = null },
            actions = {
                Button(onClick = {
                    model.restoreVersion(id, text, tr("Restored the version of {date}").replace("{date}", whenText(v.modified)))
                    back()
                }, Modifier.padding(end = 8.dp)) { Text(tr("Restore")) }
            },
        ) {
            SelectionContainer {
                Text(
                    text,
                    Modifier.padding(16.dp).horizontalScroll(rememberScrollState()),
                    fontFamily = FontFamily.Monospace,
                    style = MaterialTheme.typography.bodyMedium,
                )
            }
        }
        return
    }
    Frame(note?.title ?: tr("Versions"), back) {
        var asked by remember { mutableStateOf<Asked<List<Version>>>(Asked.Loading) }
        LaunchedEffect(id) {
            asked = withContext(Dispatchers.IO) {
                try {
                    val n = core.note(id) ?: return@withContext Asked.Got(emptyList())
                    runCatching { core.syncAccount(n.account) }
                    Asked.Got(core.versions(id))
                } catch (e: Exception) {
                    Asked.Failed(reason(e))
                }
            }
        }
        val versions = (asked as? Asked.Got)?.value
        // Only the note as it is: nothing earlier to restore.
        if (versions == null || versions.all { it.current }) {
            Status(
                asked, Icons.Outlined.History, tr("Cannot Load the Versions"), tr("No Versions Yet"),
                tr("Versions are kept once the note is on your Nextcloud and changes there"),
            )
            return@Frame
        }
        val scope = rememberCoroutineScope()
        var opening by remember { mutableStateOf<String?>(null) }
        Note(tr("Your Nextcloud keeps earlier versions as its administrator set it: by default, fewer the older they are."))
        Section(null) {
            for (v in versions) {
                val details = listOfNotNull(
                    tr("Current version").takeIf { v.current },
                    v.label?.takeIf { it.isNotEmpty() },
                    v.author?.takeIf { it.isNotEmpty() },
                ).joinToString(" · ")
                Item(
                    headlineContent = { Text(whenText(v.modified)) },
                    supportingContent = details.takeIf { it.isNotEmpty() }?.let { { Text(it) } },
                    trailingContent = if (v.current) null else ({ Icon(Icons.AutoMirrored.Filled.KeyboardArrowRight, null) }),
                    modifier = if (v.current) Modifier else Modifier.clickable(enabled = opening == null) {
                        opening = v.id
                        scope.launch {
                            try {
                                val text = withContext(Dispatchers.IO) { core.versionText(note!!.account, v.id) }
                                shown = v to text
                            } catch (e: Exception) {
                                model.messages.tryEmit(Message(reason(e)))
                            }
                            opening = null
                        }
                    },
                )
            }
        }
    }
}
