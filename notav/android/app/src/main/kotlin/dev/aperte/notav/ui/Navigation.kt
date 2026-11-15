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
import androidx.compose.material3.IconToggleButton
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
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.PathFillType
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.path
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import dev.aperte.notav.Main
import dev.aperte.notav.Screen
import dev.aperte.notav.tr
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/** The sidebar button's icon: the window's outline with a panel at its left, filled while the sidebar shows. */
private fun sidebarIcon(shown: Boolean) = ImageVector.Builder("Sidebar", 24.dp, 24.dp, 24f, 24f).apply {
    // The outline, 2 thick, and the line between the panel and the rest.
    path(fill = SolidColor(Color.Black), pathFillType = PathFillType.EvenOdd) {
        moveTo(5f, 3f); horizontalLineTo(19f); arcTo(2f, 2f, 0f, false, true, 21f, 5f)
        verticalLineTo(19f); arcTo(2f, 2f, 0f, false, true, 19f, 21f)
        horizontalLineTo(5f); arcTo(2f, 2f, 0f, false, true, 3f, 19f)
        verticalLineTo(5f); arcTo(2f, 2f, 0f, false, true, 5f, 3f); close()
        moveTo(5f, 5f); verticalLineTo(19f); horizontalLineTo(19f); verticalLineTo(5f); close()
    }
    path(fill = SolidColor(Color.Black)) {
        moveTo(8f, 5f); horizontalLineTo(10f); verticalLineTo(19f); horizontalLineTo(8f); close()
    }
    if (shown) {
        path(fill = SolidColor(Color.Black)) {
            moveTo(5f, 5f); horizontalLineTo(8f); verticalLineTo(19f); horizontalLineTo(5f); close()
        }
    }
}.build()

private val SidebarShown = sidebarIcon(true)
private val SidebarHidden = sidebarIcon(false)

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
    // Beside the note, whether the tree's pane shows (the menu button at the rail's top).
    var pane by rememberSaveable { mutableStateOf(true) }
    BoxWithConstraints(Modifier.fillMaxSize()) {
        val rail = maxWidth >= TWO_PANES + RAIL_WIDTH
        if (rail) {
            Row(Modifier.fillMaxSize()) {
                Rail(model, screen, recent, pane, { pane = !pane }) {
                    recent = it
                    pane = true
                }
                // The rail took the start inset; the panes beside it need none of their own.
                Box(Modifier.weight(1f).consumeWindowInsets(WindowInsets.systemBars.only(WindowInsetsSides.Start))) {
                    Panes(model, screen, wide = true, recent = recent, shown = pane, bar = null)
                }
            }
        } else {
            Panes(model, screen, wide = false, recent = recent, shown = true) { Bar(model, screen, recent) { recent = it } }
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

/**
 * The sidebar button at the top (the tree's pane shown or hidden), Notes and Recent as icons in the
 * middle, the add button at the bottom.
 */
@Composable
private fun Rail(
    model: Main,
    screen: Screen,
    recent: Boolean,
    pane: Boolean,
    onPane: () -> Unit,
    onList: (Boolean) -> Unit,
) {
    NavigationRail(
        header = {
            Tip(tr("Toggle Sidebar")) {
                IconToggleButton(checked = pane, onCheckedChange = { onPane() }) {
                    Icon(if (pane) SidebarShown else SidebarHidden, tr("Toggle Sidebar"))
                }
            }
        },
    ) {
        Spacer(Modifier.weight(1f))
        RailIcon(!recent, tr("Notes"), if (recent) Icons.AutoMirrored.Outlined.Notes else Icons.AutoMirrored.Filled.Notes) { onList(false) }
        RailIcon(recent, tr("Recent"), if (recent) Icons.Filled.History else Icons.Outlined.History) { onList(true) }
        Spacer(Modifier.weight(1f))
        AddNote(model, screen, raised = true)
    }
}

/** A rail's destination as an icon alone, its name as the tooltip and for screen readers. */
@Composable
private fun RailIcon(selected: Boolean, label: String, icon: ImageVector, onClick: () -> Unit) {
    Tip(label) {
        NavigationRailItem(
            selected = selected,
            onClick = onClick,
            icon = { Icon(icon, label) },
        )
    }
}

/**
 * Where a new note goes by default, with the folder's name to show: at the top, the notes folder's
 * own name, not its path on the server.
 */
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
            model.defaultPlace()?.let { (a, f) -> Place(a, f, f.ifEmpty { model.core.notesFolder(a).trim('/').substringAfterLast('/') }) }
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
