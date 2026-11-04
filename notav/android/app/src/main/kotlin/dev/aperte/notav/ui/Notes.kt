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
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.text.input.TextFieldLineLimits
import androidx.compose.foundation.text.input.rememberTextFieldState
import androidx.compose.material3.TextField
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.text.TextRange
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowRight
import androidx.compose.material.icons.automirrored.outlined.NoteAdd
import androidx.compose.material.icons.automirrored.outlined.Notes
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.outlined.CreateNewFolder
import androidx.compose.material.icons.filled.KeyboardArrowDown
import androidx.compose.material.icons.filled.Star
import androidx.compose.material.icons.outlined.Folder
import androidx.compose.material.icons.outlined.Search
import androidx.compose.material.icons.outlined.Settings
import androidx.compose.material.icons.outlined.Today
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.outlined.DriveFileMove
import androidx.compose.material.icons.outlined.Delete
import androidx.compose.material.icons.outlined.Edit
import androidx.compose.material.icons.outlined.StarBorder
import androidx.compose.material.icons.outlined.History
import androidx.compose.material.icons.outlined.MoreVert
import androidx.compose.material3.IconButton
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.material3.pulltorefresh.PullToRefreshDefaults
import androidx.compose.material3.pulltorefresh.rememberPullToRefreshState
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.ui.input.pointer.PointerEventPass
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.runtime.derivedStateOf
import dev.aperte.notav.core.AccountKind
import dev.aperte.notav.core.Locked
import dev.aperte.notav.titleOf
import java.text.DateFormat
import java.util.Date
import kotlinx.coroutines.delay
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.snapshotFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.withTimeoutOrNull
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.FloatingActionButtonMenu
import androidx.compose.material3.FloatingActionButtonMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.ToggleFloatingActionButton
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

/** The notes: the tree, the open note and its side panel, swiped between (`Panes`). */
@Composable
fun Home(model: Main, screen: Screen) = Panes(model, screen)

/**
 * `content` (the notes or a settings page) with the model's messages over it: an undo, a failed
 * sync, conflict copies, as snackbars.
 */
@Composable
fun WithMessages(model: Main, content: @Composable () -> Unit) {
    val snackbar = remember { SnackbarHostState() }
    Messages(model, snackbar)
    Box(Modifier.fillMaxSize()) {
        content()
        // Over the keyboard too.
        SnackbarHost(snackbar, Modifier.align(Alignment.BottomCenter).navigationBarsPadding().imePadding())
    }
}

@Composable
private fun Messages(model: Main, snackbar: SnackbarHostState) {
    // The lock whose Unlock… was pressed: asked first.
    var unlocking by remember { mutableStateOf<Locked?>(null) }
    // The locks told about already: each is told once, again after Unlock.
    val told = remember { mutableSetOf<Locked>() }
    // The latest message replaces the one shown; one with Undo stays a while longer, a lock until
    // it is dismissed.
    LaunchedEffect(Unit) {
        model.messages.collectLatest { m ->
            snackbar.currentSnackbarData?.dismiss()
            val r = snackbar.showSnackbar(
                m.text,
                actionLabel = m.undo?.let { tr("Undo") } ?: m.unlock?.let { tr("Unlock…") },
                withDismissAction = true,
                duration = when {
                    m.unlock != null -> SnackbarDuration.Indefinite
                    m.undo != null -> SnackbarDuration.Long
                    else -> SnackbarDuration.Short
                },
            )
            if (r == SnackbarResult.ActionPerformed) {
                if (m.undo != null) model.undo(m.undo) else unlocking = m.unlock
            }
        }
    }
    // A change waiting for a lock on the server: who holds it and since when, as the Linux app's
    // toast; Unlock… for a note's.
    LaunchedEffect(Unit) {
        model.notav.locked.collect { locks ->
            told.retainAll(locks.toSet())
            for (l in locks.filter { it !in told }) {
                told += l
                val since = l.since?.let {
                    DateFormat.getDateTimeInstance(DateFormat.SHORT, DateFormat.SHORT).format(Date(it * 1000))
                }
                val text = when {
                    l.file == null -> tr("Something in “{title}” is locked on the server")
                    l.owner != null && since != null -> tr("“{title}” is locked on the server by {owner} since {time}")
                    l.owner != null -> tr("“{title}” is locked on the server by {owner}")
                    else -> tr("“{title}” is locked on the server")
                }.replace("{title}", titleOf(l.path)).replace("{owner}", l.owner.orEmpty()).replace("{time}", since.orEmpty())
                model.messages.tryEmit(dev.aperte.notav.Message(text, unlock = l.takeIf { it.file != null }))
            }
        }
    }
    unlocking?.let { l ->
        // An editor that still has the note open may lose what it saves afterwards.
        AlertDialog(
            onDismissRequest = { unlocking = null },
            title = { Text(tr("Unlock “{title}”?").replace("{title}", titleOf(l.path))) },
            text = {
                Text(
                    tr(
                        "Your change to it waits for the lock. If it is still open in an editor, what is saved there " +
                            "afterwards may be lost: unlock it only if nobody is editing it.",
                    ),
                )
            },
            confirmButton = {
                TextButton(onClick = {
                    unlocking = null
                    told.remove(l)
                    model.unlock(l)
                }) { Text(tr("Unlock"), color = MaterialTheme.colorScheme.error) }
            },
            dismissButton = { TextButton(onClick = { unlocking = null }) { Text(tr("Cancel")) } },
        )
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
internal fun TreePane(model: Main, screen: Screen) {
    val sync by model.notav.syncState.collectAsStateWithLifecycle()
    val found by model.found.collectAsStateWithLifecycle()
    var ask by remember { mutableStateOf<Ask?>(null) }
    val changes by model.notav.changes.collectAsStateWithLifecycle()
    val look by produceState(TreeLook(), changes) {
        value = withContext(Dispatchers.IO) { TreeLook(model.core.setting("dense_tree") == "true", model.core.setting("tree_icons")) }
    }
    // Pulled to refresh (as Todav's list): shown until the sync it asked for has run, once,
    // without dropping in between (the sync starts a moment after the pull ends).
    var pulling by remember { mutableStateOf(false) }
    var started by remember { mutableStateOf(false) }
    LaunchedEffect(sync, pulling) {
        when {
            !pulling -> started = false
            sync == SyncState.Syncing -> started = true
            started -> pulling = false
            else -> {
                // No sync came of it: not held up for good.
                delay(3000)
                pulling = false
            }
        }
    }
    val canPull = screen.accounts.any { it.kind == AccountKind.NEXTCLOUD && it.enabled }
    val pull = rememberPullToRefreshState()
    val list = rememberLazyListState()
    // Only from a gesture that starts with the tree at its top and pulls straight down: one that
    // scrolled the tree on its way (down, then back up to the top) stops there.
    var fromTop by remember { mutableStateOf(true) }
    // Felt as it is pulled far enough to sync once let go.
    val haptics = LocalHapticFeedback.current
    val armed by remember { derivedStateOf { pull.distanceFraction >= 1f } }
    LaunchedEffect(armed) {
        if (armed && !pulling) haptics.performHapticFeedback(HapticFeedbackType.GestureThresholdActivate)
    }
    Scaffold(
        topBar = {
            val results = found
            if (results != null) {
                BackHandler { model.search(null) }
                val query = rememberTextFieldState()
                val focus = remember { FocusRequester() }
                LaunchedEffect(Unit) { focus.requestFocus() }
                LaunchedEffect(query.text) { model.search(query.text.toString()) }
                TopAppBar(
                    navigationIcon = {
                        IconButton(onClick = { model.search(null) }) {
                            Icon(Icons.AutoMirrored.Filled.ArrowBack, tr("Cancel"))
                        }
                    },
                    title = {
                        androidx.compose.foundation.text.BasicTextField(
                            query,
                            Modifier.fillMaxWidth().focusRequester(focus),
                            textStyle = MaterialTheme.typography.titleLarge.copy(color = MaterialTheme.colorScheme.onSurface),
                            lineLimits = TextFieldLineLimits.SingleLine,
                            cursorBrush = androidx.compose.ui.graphics.SolidColor(MaterialTheme.colorScheme.primary),
                            decorator = { field ->
                                if (query.text.isEmpty()) {
                                    Text(
                                        tr("Search notes"),
                                        style = MaterialTheme.typography.titleLarge,
                                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                                    )
                                }
                                field()
                            },
                        )
                    },
                )
            } else {
                TopAppBar(
                    title = { Text("Notav") },
                    actions = {
                        if (sync == SyncState.Syncing && !pulling) LoadingIndicator(Modifier.padding(end = 4.dp).size(32.dp))
                        Tip(tr("Today’s Note")) {
                            IconButton(onClick = { model.today() }) { Icon(Icons.Outlined.Today, tr("Today’s Note")) }
                        }
                        Tip(tr("Search Notes")) {
                            IconButton(onClick = { model.search("") }) { Icon(Icons.Outlined.Search, tr("Search Notes")) }
                        }
                        // The Trash (where a server keeps deleted notes) and Settings, as the
                        // Linux app's main menu.
                        var more by remember { mutableStateOf(false) }
                        Box {
                            Tip(tr("Main Menu")) {
                                IconButton(onClick = { more = true }) { Icon(Icons.Outlined.MoreVert, tr("Main Menu")) }
                            }
                            val trash = remember(more, screen.accounts) { more && model.trashAccounts().isNotEmpty() }
                            Menu(
                                more,
                                { more = false },
                                listOf(
                                    listOfNotNull(
                                        Entry(tr("Trash"), Icons.Outlined.Delete) { model.page.value = dev.aperte.notav.Page.Trash }
                                            .takeIf { trash },
                                        Entry(tr("Settings"), Icons.Outlined.Settings) { model.page.value = dev.aperte.notav.Page.Settings },
                                    ),
                                ),
                            )
                        }
                    },
                )
            }
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
        val results = found
        if (results != null) {
            // The notes found, each with its folder (and account, with more than one).
            if (results.isEmpty()) {
                Box(Modifier.fillMaxSize().padding(padding), Alignment.Center) {
                    Text(tr("No Results"), style = MaterialTheme.typography.titleLarge)
                }
            } else {
                val names = screen.accounts.associate { it.id to accountName(it) }
                LazyColumn(Modifier.fillMaxSize().padding(padding)) {
                    items(results, key = { it.id }) { n ->
                        val where = listOfNotNull(
                            names[n.account].takeIf { screen.folders.size > 1 },
                            n.folder.takeIf { it.isNotEmpty() },
                        ).joinToString(" · ")
                        Item(
                            headlineContent = { Text(n.title, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                            supportingContent = where.takeIf { it.isNotEmpty() }?.let { { Text(it) } },
                            leadingContent = { Icon(Icons.AutoMirrored.Outlined.Notes, null) },
                            modifier = Modifier.clickable { model.openNote(n) },
                            containerColor = MaterialTheme.colorScheme.surface,
                        )
                    }
                }
            }
        } else {
            PullToRefreshBox(
                isRefreshing = pulling,
                onRefresh = {
                    pulling = true
                    model.notav.syncNow()
                },
                modifier = Modifier.fillMaxSize().padding(padding).pointerInput(Unit) {
                    awaitEachGesture {
                        awaitFirstDown(requireUnconsumed = false, pass = PointerEventPass.Initial)
                        fromTop = !list.canScrollBackward
                        do {
                            val e = awaitPointerEvent(PointerEventPass.Initial)
                            if (list.canScrollBackward) fromTop = false
                        } while (e.changes.any { it.pressed })
                    }
                },
                state = pull,
                enabled = canPull && (fromTop || pulling),
                indicator = { PullToRefreshDefaults.LoadingIndicator(pull, pulling, Modifier.align(Alignment.TopCenter)) },
            ) {
                // The note opened scrolled into view, once its row is there (a new one's comes a
                // moment later); not again as other rows come and go (a folder opened).
                val open by model.open.collectAsStateWithLifecycle()
                val rows by rememberUpdatedState(screen.rows)
                LaunchedEffect(open) {
                    val i = withTimeoutOrNull(2000) {
                        snapshotFlow { rows.indexOfFirst { (it.item as? Item.NoteRow)?.note?.id == open } }.first { it >= 0 }
                    } ?: return@LaunchedEffect
                    if (list.layoutInfo.visibleItemsInfo.none { it.index == i && it.offset >= 0 }) {
                        list.animateScrollToItem(i)
                    }
                }
                // Empty, a list too: pulled the same.
                LazyColumn(Modifier.fillMaxSize(), state = list) {
                    if (screen.rows.isEmpty()) {
                        item {
                            Box(Modifier.fillParentMaxSize(), Alignment.Center) {
                                Text(tr("No Notes"), style = MaterialTheme.typography.titleLarge)
                            }
                        }
                    } else {
                        items(screen.rows, key = { r -> rowKey(r.item) }) { r ->
                            TreeRow(model, screen, look, r.depth, r.item) { ask = it }
                        }
                        item { Spacer(Modifier.height(96.dp)) } // room under the button
                    }
                }
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

/**
 * How the tree looks, as the Linux app's: shorter rows with the device-only `dense_tree`; the
 * synced `tree_icons`, "all" (folders too), "notes" (the default) or "none".
 */
private data class TreeLook(val dense: Boolean = false, val icons: String? = null) {
    val folderIcons get() = icons == "all"
    val noteIcons get() = icons != "none"
}

private fun rowKey(item: Item): String = when (item) {
    is Item.Account -> "a${item.id}"
    is Item.Folder -> "f${item.account}:${item.path}"
    is Item.NoteRow -> "n${item.note.id}"
}

/** A row of the tree: a tap opens a note or opens and closes a folder, a long press its menu. */
@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun TreeRow(model: Main, screen: Screen, look: TreeLook, depth: Int, item: Item, ask: (Ask) -> Unit) {
    val expanded by model.expanded.collectAsStateWithLifecycle()
    val open by model.open.collectAsStateWithLifecycle()
    var menu by remember { mutableStateOf(false) }
    val (account, folder) = item.place
    val selected = item is Item.NoteRow && item.note.id == open
    Box {
        Row(
            Modifier.fillMaxWidth().heightIn(min = if (look.dense) 40.dp else 52.dp)
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
                    if (look.folderIcons && item is Item.Folder) Icon(Icons.Outlined.Folder, null)
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
                    if (look.noteIcons) Icon(Icons.AutoMirrored.Outlined.Notes, null, tint = color)
                    Text(item.note.title, Modifier.weight(1f), color = color, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    if (item.note.favorite) {
                        Icon(Icons.Filled.Star, tr("Favorite"), Modifier.size(18.dp), tint = MaterialTheme.colorScheme.primary)
                    }
                }
            }
        }
        val note = (item as? Item.NoteRow)?.note
        val owned = item !is Item.Account
        // Asked of the account only once the menu opens.
        val versions = remember(menu, note) { menu && note != null && model.hasVersions(note) }
        Menu(
            menu,
            { menu = false },
            listOf(
                if (note == null) {
                    listOf(
                        Entry(tr("New Note"), Icons.AutoMirrored.Outlined.NoteAdd) { model.newNote(item.place) },
                        Entry(tr("New Folder"), Icons.Outlined.CreateNewFolder) { ask(Ask.NewFolder(item.place)) },
                    )
                } else {
                    listOfNotNull(
                        if (note.favorite) {
                            Entry(tr("Remove from Favorites"), Icons.Filled.Star) { model.setFavorite(note, false) }
                        } else {
                            Entry(tr("Add to Favorites"), Icons.Outlined.StarBorder) { model.setFavorite(note, true) }
                        },
                        Entry(tr("Versions…"), Icons.Outlined.History) { model.page.value = dev.aperte.notav.Page.Versions(note.id) }
                            .takeIf { versions },
                    )
                },
                if (owned) {
                    listOf(
                        Entry(tr("Rename…"), Icons.Outlined.Edit) { ask(Ask.Rename(item)) },
                        Entry(tr("Move…"), Icons.AutoMirrored.Outlined.DriveFileMove) { ask(Ask.Move(item)) },
                    )
                } else {
                    emptyList()
                },
                if (owned) listOf(Entry(tr("Delete"), Icons.Outlined.Delete, danger = true) { model.delete(item) }) else emptyList(),
            ),
        )
    }
}

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
            // A block of rows, as Todav's Move to List.
            LazyColumn(Modifier.clip(RoundedCornerShape(24.dp)), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                items(targets.filter { moves(item, it) }) { f ->
                    Item(
                        headlineContent = { Text(if (f.isEmpty()) top else f) },
                        leadingContent = { Icon(Icons.Outlined.Folder, null) },
                        modifier = Modifier.clickable { onPick(f) },
                        containerColor = MaterialTheme.colorScheme.surfaceContainerHighest,
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
internal fun NoNote() =androidx.compose.material3.Surface(Modifier.fillMaxSize()) {
    Column(
        Modifier.fillMaxSize().padding(32.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp, Alignment.CenterVertically),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Text(tr("No Note Selected"), style = MaterialTheme.typography.titleLarge, textAlign = TextAlign.Center)
    }
}
