package dev.aperte.notav.ui

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.ui.draw.clip
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.text.input.TextFieldLineLimits
import androidx.compose.foundation.text.input.rememberTextFieldState
import androidx.compose.material3.TextField
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.text.TextRange
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowRight
import androidx.compose.material.icons.automirrored.outlined.NoteAdd
import androidx.compose.material.icons.automirrored.outlined.Notes
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.outlined.CreateNewFolder
import androidx.compose.material.icons.filled.KeyboardArrowDown
import androidx.compose.material.icons.filled.Star
import androidx.compose.material.icons.outlined.Folder
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.FloatingActionButtonMenu
import androidx.compose.material3.FloatingActionButtonMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.ToggleFloatingActionButton
import androidx.compose.material3.IconButton
import androidx.compose.material3.LoadingIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarDuration
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import kotlinx.coroutines.flow.collectLatest
import androidx.compose.material3.SnackbarResult
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.VerticalDivider
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import dev.aperte.notav.Item
import dev.aperte.notav.Main
import dev.aperte.notav.Screen
import dev.aperte.notav.SyncState
import dev.aperte.notav.accountName
import dev.aperte.notav.moves
import dev.aperte.notav.notesIn
import dev.aperte.notav.ntr
import dev.aperte.notav.tr
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/** The narrowest window that shows the tree and the note side by side. */
private val TWO_PANES = 600.dp
private val TREE_WIDTH = 360.dp

/**
 * The notes: on a phone the tree, then the note opened from it on a page of its own; on a wider
 * screen both side by side. Messages (an undo, a failed sync) in a snackbar over both.
 */
@Composable
fun Home(model: Main, screen: Screen) {
    val open by model.open.collectAsStateWithLifecycle()
    val note = screen.notes.firstOrNull { it.id == open }
    val snackbar = remember { SnackbarHostState() }
    Messages(model, snackbar)
    Box(Modifier.fillMaxSize()) {
        BoxWithConstraints(Modifier.fillMaxSize()) {
            if (maxWidth >= TWO_PANES) {
                Row(Modifier.fillMaxSize()) {
                    Box(Modifier.width(TREE_WIDTH).fillMaxHeight()) { TreePane(model, screen) }
                    VerticalDivider()
                    Box(Modifier.weight(1f).fillMaxHeight()) {
                        if (note != null) NotePane(model, note, back = false) else NoNote()
                    }
                }
            } else if (note != null) {
                BackHandler { model.open.value = null }
                NotePane(model, note, back = true)
            } else {
                TreePane(model, screen)
            }
        }
        SnackbarHost(snackbar, Modifier.align(Alignment.BottomCenter).navigationBarsPadding())
    }
}

/** The model's messages, a failed sync and conflict copies, as snackbars. */
@Composable
private fun Messages(model: Main, snackbar: SnackbarHostState) {
    // The latest message replaces the one shown; one with Undo stays a while longer.
    LaunchedEffect(Unit) {
        model.messages.collectLatest { m ->
            snackbar.currentSnackbarData?.dismiss()
            val r = snackbar.showSnackbar(
                m.text,
                actionLabel = m.undo?.let { tr("Undo") },
                withDismissAction = true,
                duration = if (m.undo != null) SnackbarDuration.Long else SnackbarDuration.Short,
            )
            if (r == SnackbarResult.ActionPerformed) model.undo(m.undo)
        }
    }
    LaunchedEffect(Unit) {
        var last: String? = null
        model.notav.syncState.collect { s ->
            when {
                s is SyncState.Failed && s.message != last -> {
                    last = s.message
                    model.messages.tryEmit(dev.aperte.notav.Message(tr("Sync failed: {error}").replace("{error}", s.message)))
                }
                s is SyncState.Idle -> last = null
            }
        }
    }
    LaunchedEffect(Unit) {
        model.notav.conflicts.collect { copies ->
            val text = if (copies.size == 1) {
                tr("“{title}” was changed elsewhere too: your version is kept next to it as a copy")
                    .replace("{title}", copies[0].substringAfterLast('/').removeSuffix(".md"))
            } else {
                ntr(
                    "{n} note was changed elsewhere too: your version is kept next to it as a copy",
                    "{n} notes were changed elsewhere too: your versions are kept next to them as copies",
                    copies.size.toLong(),
                ).replace("{n}", copies.size.toString())
            }
            model.messages.tryEmit(dev.aperte.notav.Message(text))
        }
    }
}

/** What an action on a row of the tree asks for first. */
private sealed interface Ask {
    data class NewFolder(val place: Pair<Long, String>) : Ask
    data class Rename(val item: Item) : Ask
    data class Move(val item: Item) : Ask
}

@OptIn(ExperimentalMaterial3Api::class, ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun TreePane(model: Main, screen: Screen) {
    val sync by model.notav.syncState.collectAsStateWithLifecycle()
    var ask by remember { mutableStateOf<Ask?>(null) }
    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Notav") },
                actions = {
                    if (sync == SyncState.Syncing) LoadingIndicator(Modifier.padding(end = 12.dp).size(32.dp))
                },
            )
        },
        floatingActionButton = {
            // A new note or folder where the last one went (the folder last opened, or the open
            // note's); a folder's menu puts one in that folder.
            var adding by rememberSaveable { mutableStateOf(false) }
            BackHandler(adding) { adding = false }
            FloatingActionButtonMenu(
                expanded = adding,
                button = {
                    Tip(tr("Add")) {
                        ToggleFloatingActionButton(checked = adding, onCheckedChange = { adding = it }) {
                            Icon(if (adding) Icons.Filled.Close else Icons.Filled.Add, tr("Add"))
                        }
                    }
                },
            ) {
                FloatingActionButtonMenuItem(
                    onClick = { adding = false; model.newNote() },
                    text = { Text(tr("New Note")) },
                    icon = { Icon(Icons.AutoMirrored.Outlined.NoteAdd, null) },
                )
                FloatingActionButtonMenuItem(
                    onClick = { adding = false; model.newPlace()?.let { ask = Ask.NewFolder(it) } },
                    text = { Text(tr("New Folder")) },
                    icon = { Icon(Icons.Outlined.CreateNewFolder, null) },
                )
            }
        },
    ) { padding ->
        if (screen.rows.isEmpty()) {
            Box(Modifier.fillMaxSize().padding(padding), Alignment.Center) {
                Text(tr("No Notes"), style = MaterialTheme.typography.titleLarge)
            }
        } else {
            // The note opened (a new one too) scrolled into view.
            val list = rememberLazyListState()
            val open by model.open.collectAsStateWithLifecycle()
            LaunchedEffect(open, screen.rows.size) {
                val i = screen.rows.indexOfFirst { (it.item as? Item.NoteRow)?.note?.id == open }
                if (i >= 0 && list.layoutInfo.visibleItemsInfo.none { it.index == i && it.offset >= 0 }) {
                    list.animateScrollToItem(i)
                }
            }
            LazyColumn(Modifier.fillMaxSize().padding(padding), state = list) {
                items(screen.rows, key = { r -> rowKey(r.item) }) { r ->
                    TreeRow(model, screen, r.depth, r.item) { ask = it }
                }
                item { Spacer(Modifier.height(96.dp)) } // room under the button
            }
        }
    }
    when (val a = ask) {
        is Ask.NewFolder -> NameDialog(tr("New Folder"), tr("Create"), "", { ask = null }) {
            ask = null
            model.newFolder(a.place, it)
        }
        is Ask.Rename -> {
            val (heading, current) = when (val i = a.item) {
                is Item.Folder -> tr("Rename Folder") to i.name
                is Item.NoteRow -> tr("Rename") to i.note.title
                is Item.Account -> return
            }
            NameDialog(heading, tr("Rename"), current, { ask = null }) {
                ask = null
                when (val i = a.item) {
                    is Item.Folder -> model.renameFolder(i, it)
                    is Item.NoteRow -> model.renameNote(i.note, it)
                    is Item.Account -> {}
                }
            }
        }
        is Ask.Move -> MoveDialog(model, screen, a.item, { ask = null }) {
            ask = null
            model.move(a.item, it)
        }
        null -> {}
    }
}

private fun rowKey(item: Item): String = when (item) {
    is Item.Account -> "a${item.id}"
    is Item.Folder -> "f${item.account}:${item.path}"
    is Item.NoteRow -> "n${item.note.id}"
}

/** A row of the tree: a tap opens a note or opens and closes a folder, a long press its menu. */
@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun TreeRow(model: Main, screen: Screen, depth: Int, item: Item, ask: (Ask) -> Unit) {
    val expanded by model.expanded.collectAsStateWithLifecycle()
    val open by model.open.collectAsStateWithLifecycle()
    var menu by remember { mutableStateOf(false) }
    val (account, folder) = item.place
    val selected = item is Item.NoteRow && item.note.id == open
    Box {
        Row(
            Modifier.fillMaxWidth().heightIn(min = 52.dp)
                .padding(horizontal = 8.dp)
                .clip(RoundedCornerShape(16.dp))
                .then(if (selected) Modifier.background(MaterialTheme.colorScheme.secondaryContainer) else Modifier)
                .combinedClickable(
                    onClick = { if (item is Item.NoteRow) model.openNote(item.note) else model.toggle(item) },
                    onLongClick = { menu = true },
                )
                .padding(start = 8.dp + 20.dp * depth, end = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            val color = if (selected) MaterialTheme.colorScheme.onSecondaryContainer else MaterialTheme.colorScheme.onSurface
            when (item) {
                is Item.Account, is Item.Folder -> {
                    val isOpen = when (item) {
                        is Item.Account -> dev.aperte.notav.folderKey(account, "") !in expanded
                        else -> dev.aperte.notav.folderKey(account, folder) in expanded
                    }
                    Icon(
                        if (isOpen) Icons.Filled.KeyboardArrowDown else Icons.AutoMirrored.Filled.KeyboardArrowRight,
                        null,
                    )
                    val name = when (item) {
                        is Item.Account -> screen.accounts.firstOrNull { it.id == item.id }?.let(::accountName).orEmpty()
                        else -> (item as Item.Folder).name
                    }
                    Text(
                        name,
                        Modifier.weight(1f),
                        fontWeight = if (item is Item.Account) FontWeight.Bold else null,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                    Text(
                        notesIn(screen.notes, account, if (item is Item.Account) "" else folder).toString(),
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        style = MaterialTheme.typography.labelLarge,
                    )
                }
                is Item.NoteRow -> {
                    Icon(Icons.AutoMirrored.Outlined.Notes, null, tint = color)
                    Text(item.note.title, Modifier.weight(1f), color = color, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    if (item.note.favorite) {
                        Icon(Icons.Filled.Star, tr("Favorite"), Modifier.size(18.dp), tint = MaterialTheme.colorScheme.primary)
                    }
                }
            }
        }
        DropdownMenu(menu, { menu = false }) {
            val close = { menu = false }
            if (item !is Item.NoteRow) {
                Entry(tr("New Note"), close) { model.newNote(item.place) }
                Entry(tr("New Folder"), close) { ask(Ask.NewFolder(item.place)) }
            }
            if (item is Item.NoteRow) {
                val n = item.note
                Entry(if (n.favorite) tr("Remove from Favorites") else tr("Add to Favorites"), close) {
                    model.setFavorite(n, !n.favorite)
                }
            }
            if (item !is Item.Account) {
                Entry(tr("Rename…"), close) { ask(Ask.Rename(item)) }
                Entry(tr("Move…"), close) { ask(Ask.Move(item)) }
                Entry(tr("Delete"), close) { model.delete(item) }
            }
        }
    }
}

@Composable
private fun Entry(label: String, close: () -> Unit, action: () -> Unit) =
    DropdownMenuItem(text = { Text(label) }, onClick = { close(); action() })

/** A name to give (a new folder, a note's or folder's new name): the field focused, the name
 * there selected to type over. */
@Composable
private fun NameDialog(title: String, confirm: String, initial: String, onDismiss: () -> Unit, onDone: (String) -> Unit) {
    val state = rememberTextFieldState(initial, TextRange(0, initial.length))
    val focus = remember { FocusRequester() }
    LaunchedEffect(Unit) { focus.requestFocus() }
    val name = state.text.toString()
    val ready = name.isNotBlank() && name.trim() != initial
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(title) },
        text = {
            TextField(
                state,
                Modifier.fillMaxWidth().focusRequester(focus),
                label = { Text(tr("Name")) },
                lineLimits = TextFieldLineLimits.SingleLine,
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
                onKeyboardAction = { if (ready) onDone(name) },
            )
        },
        confirmButton = { Button(onClick = { onDone(name) }, enabled = ready) { Text(confirm) } },
        dismissButton = { TextButton(onClick = onDismiss) { Text(tr("Cancel")) } },
    )
}

/** The folder of its account to move `item` into: the notes folder's top or one of its folders. */
@Composable
private fun MoveDialog(model: Main, screen: Screen, item: Item, onDismiss: () -> Unit, onPick: (String) -> Unit) {
    val account = item.place.first
    val top by produceState("", account) { value = withContext(Dispatchers.IO) { model.core.notesFolder(account) } }
    val targets = listOf("") + screen.folders[account].orEmpty().sortedBy { it.lowercase() }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(tr("Move")) },
        text = {
            LazyColumn {
                items(targets.filter { moves(item, it) }) { f ->
                    Item(
                        headlineContent = { Text(if (f.isEmpty()) top else f) },
                        leadingContent = { Icon(Icons.Outlined.Folder, null) },
                        modifier = Modifier.clickable { onPick(f) },
                        containerColor = MaterialTheme.colorScheme.surfaceContainerHigh,
                    )
                }
            }
        },
        confirmButton = {},
        dismissButton = { TextButton(onClick = onDismiss) { Text(tr("Cancel")) } },
    )
}

/** No note open beside the tree. */
@Composable
private fun NoNote() = androidx.compose.material3.Surface(Modifier.fillMaxSize()) {
    Column(
        Modifier.fillMaxSize().padding(32.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp, Alignment.CenterVertically),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Text(tr("No Note Selected"), style = MaterialTheme.typography.titleLarge, textAlign = TextAlign.Center)
    }
}

/** The open note. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun NotePane(model: Main, note: dev.aperte.notav.core.Note, back: Boolean) {
    val changes by model.notav.changes.collectAsStateWithLifecycle()
    val text by produceState<String?>(null, note.id, changes) {
        value = withContext(Dispatchers.IO) { model.core.noteText(note.id) }
    }
    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(note.title, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                navigationIcon = {
                    if (back) {
                        IconButton(onClick = { model.open.value = null }) {
                            Icon(Icons.AutoMirrored.Filled.ArrowBack, null)
                        }
                    }
                },
            )
        },
    ) { padding ->
        SelectionContainer(Modifier.padding(padding).fillMaxSize().verticalScroll(rememberScrollState())) {
            Text(text.orEmpty(), Modifier.padding(16.dp))
        }
    }
}
