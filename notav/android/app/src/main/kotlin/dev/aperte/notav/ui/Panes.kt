package dev.aperte.notav.ui

import androidx.activity.compose.BackHandler
import androidx.compose.animation.core.exponentialDecay
import androidx.compose.animation.core.spring
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.AnchoredDraggableState
import androidx.compose.foundation.gestures.DraggableAnchors
import androidx.compose.foundation.gestures.Orientation
import androidx.compose.foundation.gestures.anchoredDraggable
import androidx.compose.foundation.gestures.animateTo
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.wrapContentWidth
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.outlined.Notes
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.VerticalDivider
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import dev.aperte.notav.Main
import dev.aperte.notav.Screen
import dev.aperte.notav.core.Note
import dev.aperte.notav.ntr
import dev.aperte.notav.tr
import kotlin.math.roundToInt
import kotlinx.coroutines.Dispatchers
import androidx.compose.runtime.snapshotFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.coroutines.withTimeoutOrNull

/** The narrowest window that shows the tree and the note side by side. */
private val TWO_PANES = 600.dp

/** The tree's width beside the note, and the side panel's. */
private val SIDE_WIDTH = 360.dp

/** Where the panes rest: the tree, the note, the side panel (the outline). */
private enum class Pane { Tree, Note, Side }

/**
 * The tree, the open note and its side panel in a row, swiped between: on a phone one at a time,
 * each the screen's width; on a wider screen the tree and the note, or the note and the side
 * panel. Opening a note slides to it; Back goes one pane left.
 */
@Composable
fun Panes(model: Main, screen: Screen) {
    val open by model.open.collectAsStateWithLifecycle()
    val note = screen.notes.firstOrNull { it.id == open }
    BoxWithConstraints(Modifier.fillMaxSize().clipToBounds()) {
        val wide = maxWidth >= TWO_PANES
        val width = maxWidth
        val side = if (wide) SIDE_WIDTH else width
        val noteWidth = if (wide) width - SIDE_WIDTH else width
        val px = with(LocalDensity.current) { side.toPx() }
        val drag = remember {
            @Suppress("DEPRECATION")
            AnchoredDraggableState(
                initialValue = Pane.Tree,
                positionalThreshold = { it * 0.3f },
                velocityThreshold = { px / 2 },
                snapAnimationSpec = spring(),
                decayAnimationSpec = exponentialDecay(),
            )
        }
        val scope = rememberCoroutineScope()
        // Without a note, only the tree; on a wide screen the note shows beside it.
        val anchors = DraggableAnchors {
            Pane.Tree at 0f
            if (note != null) {
                if (!wide) Pane.Note at -px
                Pane.Side at if (wide) -px else -2 * px
            }
        }
        // Folded or unfolded, the same pane where it is there; else the tree.
        LaunchedEffect(anchors) {
            drag.updateAnchors(anchors, drag.targetValue.takeIf { anchors.hasPositionFor(it) } ?: Pane.Tree)
        }
        val go = { pane: Pane -> scope.launch { drag.animateTo(if (wide && pane == Pane.Note) Pane.Tree else pane) } }
        LaunchedEffect(wide) {
            model.showNote.collect {
                // A note just opened: once its pane is there.
                if (!wide) withTimeoutOrNull(1000) { snapshotFlow { drag.anchors.hasPositionFor(Pane.Note) }.first { it } }
                go(Pane.Note)
            }
        }
        LaunchedEffect(wide) {
            model.jump.collect { if (!wide) go(Pane.Note) }
        }
        // The keyboard goes with what it typed in (the search, the note) as the panes move.
        val focus = LocalFocusManager.current
        LaunchedEffect(drag.settledValue) { focus.clearFocus() }
        Row(
            Modifier.wrapContentWidth(Alignment.Start, unbounded = true)
                .width(if (wide) SIDE_WIDTH + noteWidth + SIDE_WIDTH else width * 3)
                .fillMaxHeight()
                .offset { IntOffset(drag.offset.takeIf { !it.isNaN() }?.roundToInt() ?: 0, 0) }
                .anchoredDraggable(drag, Orientation.Horizontal, enabled = note != null),
        ) {
            Box(Modifier.width(side).fillMaxHeight()) { TreePane(model, screen) }
            if (wide) VerticalDivider()
            Box(Modifier.width(noteWidth).fillMaxHeight()) {
                if (note != null) {
                    NotePane(model, note, onBack = if (wide) null else ({ go(Pane.Tree) })) { go(Pane.Side) }
                } else {
                    NoNote()
                }
            }
            if (wide) VerticalDivider()
            Box(Modifier.width(side).fillMaxHeight()) {
                if (note != null) SidePane(model, note, onBack = { go(if (wide) Pane.Tree else Pane.Note) })
            }
        }
        // Registered anew as the panes settle, so it comes before the search's own Back (the
        // latest registered goes first) while the tree is out of sight.
        key(drag.settledValue) {
            BackHandler(drag.settledValue != Pane.Tree) {
                go(if (drag.settledValue == Pane.Side && !wide) Pane.Note else Pane.Tree)
            }
        }
    }
}

/** The open note's outline (a tap goes to the heading) and the notes linking to it. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun SidePane(model: Main, note: Note, onBack: () -> Unit) {
    val headings by model.headings.collectAsStateWithLifecycle()
    val changes by model.notav.changes.collectAsStateWithLifecycle()
    val backlinks by produceState(emptyList<Note>(), note.id, changes) {
        value = withContext(Dispatchers.IO) { model.core.backlinks(note.id) }
    }
    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(tr("Outline")) },
                navigationIcon = {
                    Tip(tr("Back")) { IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Filled.ArrowBack, tr("Back")) } }
                },
            )
        },
    ) { padding ->
        LazyColumn(Modifier.fillMaxSize().padding(padding)) {
            if (headings.isEmpty()) {
                item { Hint(tr("No Headings")) }
            }
            // Indented by level, from the highest there is.
            val top = headings.minOfOrNull { it.level.toInt() } ?: 1
            items(headings) { h ->
                Text(
                    h.title,
                    Modifier.fillMaxWidth().clickable { model.jump.tryEmit(h.at.toInt()) }
                        .padding(start = 16.dp + 16.dp * (h.level.toInt() - top), end = 16.dp, top = 12.dp, bottom = 12.dp),
                    style = if (h.level.toInt() == top) MaterialTheme.typography.titleSmall else MaterialTheme.typography.bodyLarge,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            if (backlinks.isNotEmpty()) {
                item {
                    Text(
                        ntr("Linked from {count} note", "Linked from {count} notes", backlinks.size.toLong())
                            .replace("{count}", backlinks.size.toString()),
                        style = MaterialTheme.typography.titleSmall,
                        color = MaterialTheme.colorScheme.primary,
                        modifier = Modifier.padding(start = 16.dp, top = 24.dp, bottom = 8.dp),
                    )
                }
                items(backlinks, key = { it.id }) { n ->
                    Item(
                        headlineContent = { Text(n.title, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                        supportingContent = n.folder.takeIf { it.isNotEmpty() }?.let { { Text(it) } },
                        leadingContent = { Icon(Icons.AutoMirrored.Outlined.Notes, null) },
                        modifier = Modifier.clickable { model.openNote(n) },
                        containerColor = MaterialTheme.colorScheme.surface,
                    )
                }
            }
        }
    }
}

@Composable
private fun Hint(text: String) = Text(
    text,
    Modifier.padding(16.dp),
    color = MaterialTheme.colorScheme.onSurfaceVariant,
)
