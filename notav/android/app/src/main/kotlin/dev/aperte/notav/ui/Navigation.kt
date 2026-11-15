package dev.aperte.notav.ui

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.WindowInsetsSides
import androidx.compose.foundation.layout.consumeWindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.only
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.systemBars
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.Notes
import androidx.compose.material.icons.automirrored.outlined.NoteAdd
import androidx.compose.material.icons.automirrored.outlined.Notes
import androidx.compose.material.icons.filled.History
import androidx.compose.material.icons.outlined.Folder
import androidx.compose.material.icons.outlined.History
import androidx.compose.material3.Icon
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.NavigationRail
import androidx.compose.material3.NavigationRailItem
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import dev.aperte.notav.Main
import dev.aperte.notav.Screen
import dev.aperte.notav.tr
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/** A navigation rail's width. */
private val RAIL_WIDTH = 80.dp

/**
 * The notes (`Panes`) with a navigation bar under the tree on a phone: Notes (the tree) and Recent
 * (the notes opened last), and the add button at its end. From a width that fits the rail and two
 * panes the bar is a rail at the left instead, the add button at its bottom.
 */
@Composable
fun Home(model: Main, screen: Screen) {
    // Which list the tree's pane shows; kept as the device turns.
    var recent by rememberSaveable { mutableStateOf(false) }
    BoxWithConstraints(Modifier.fillMaxSize()) {
        val rail = maxWidth >= TWO_PANES + RAIL_WIDTH
        if (rail) {
            Row(Modifier.fillMaxSize()) {
                Rail(model, screen, recent) { recent = it }
                // The rail took the start inset; the panes beside it need none of their own.
                Box(Modifier.weight(1f).consumeWindowInsets(WindowInsets.systemBars.only(WindowInsetsSides.Start))) {
                    Panes(model, screen, wide = true, recent = recent, bar = null)
                }
            }
        } else {
            Panes(model, screen, wide = false, recent = recent) { Bar(model, screen, recent) { recent = it } }
        }
    }
}

/** Notes and Recent, then the add button, under the tree. */
@Composable
private fun Bar(model: Main, screen: Screen, recent: Boolean, onList: (Boolean) -> Unit) {
    NavigationBar {
        NavigationBarItem(
            selected = !recent,
            onClick = { onList(false) },
            icon = { Icon(if (recent) Icons.AutoMirrored.Outlined.Notes else Icons.AutoMirrored.Filled.Notes, null) },
            label = { Text(tr("Notes")) },
        )
        NavigationBarItem(
            selected = recent,
            onClick = { onList(true) },
            icon = { Icon(if (recent) Icons.Filled.History else Icons.Outlined.History, null) },
            label = { Text(tr("Recent")) },
        )
        // At the end, as a bottom app bar has it, to take less room.
        Box(Modifier.padding(end = 12.dp)) { AddNote(model, screen, raised = false) }
    }
}

/** Notes and Recent, the add button at the bottom. */
@Composable
private fun Rail(model: Main, screen: Screen, recent: Boolean, onList: (Boolean) -> Unit) {
    NavigationRail {
        NavigationRailItem(
            selected = !recent,
            onClick = { onList(false) },
            icon = { Icon(if (recent) Icons.AutoMirrored.Outlined.Notes else Icons.AutoMirrored.Filled.Notes, null) },
            label = { Text(tr("Notes")) },
        )
        NavigationRailItem(
            selected = recent,
            onClick = { onList(true) },
            icon = { Icon(if (recent) Icons.Filled.History else Icons.Outlined.History, null) },
            label = { Text(tr("Recent")) },
        )
        Spacer(Modifier.weight(1f))
        AddNote(model, screen, raised = true)
    }
}

/** Where a new note goes by default, with the folder's name to show: the notes folder's own at the top. */
private data class Place(val account: Long, val folder: String, val name: String)

/**
 * The add button: a new note in the default folder (Settings) when no note is open; with one
 * open, a menu to make it next to that note or in the default folder (no menu where those are the
 * same folder). None while no account is switched on.
 */
@Composable
private fun AddNote(model: Main, screen: Screen, raised: Boolean) {
    val open by model.open.collectAsStateWithLifecycle()
    val changes by model.notav.changes.collectAsStateWithLifecycle()
    val note = screen.notes.firstOrNull { it.id == open }
    var menu by remember { mutableStateOf(false) }
    // Read again as the setting or the folders change (synced from another device too).
    val default by produceState<Place?>(null, changes, screen.accounts) {
        value = withContext(Dispatchers.IO) {
            model.defaultPlace()?.let { (a, f) -> Place(a, f, f.ifEmpty { model.core.notesFolder(a) }) }
        }
    }
    val place = default ?: return
    val here = note?.let { it.account to it.folder }
    val label = tr("New Note")
    Box {
        AddButton(label, raised) {
            if (here == null || here == (place.account to place.folder)) model.newDefaultNote() else menu = true
        }
        if (note != null && here != null) {
            // A long title cut short, as the menu is not as wide as the screen.
            val title = if (note.title.length > 24) note.title.take(24) + "…" else note.title
            Menu(
                menu,
                { menu = false },
                listOf(
                    listOf(
                        Entry(tr("New Note Next to “{title}”").replace("{title}", title), Icons.AutoMirrored.Outlined.NoteAdd) {
                            model.newNote(here)
                        },
                        Entry(tr("New Note in “{folder}”").replace("{folder}", place.name), Icons.Outlined.Folder) {
                            model.newDefaultNote()
                        },
                    ),
                ),
            )
        }
    }
}
