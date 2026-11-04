package dev.aperte.notav.ui

import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.gestures.waitForUpOrCancellation
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.isImeVisible
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.text.input.InputTransformation
import androidx.compose.foundation.text.input.OutputTransformation
import androidx.compose.foundation.text.input.TextFieldBuffer
import androidx.compose.foundation.text.input.TextFieldLineLimits
import androidx.compose.foundation.text.input.TextFieldState
import androidx.compose.foundation.text.input.rememberTextFieldState
import androidx.compose.foundation.text.input.setTextAndPlaceCursorAtEnd
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.outlined.Code
import androidx.compose.material.icons.outlined.TextFields
import androidx.compose.material3.ColorScheme
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.input.pointer.PointerEventPass
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextLayoutResult
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import dev.aperte.notav.Main
import dev.aperte.notav.core.Alert
import dev.aperte.notav.core.Note
import dev.aperte.notav.core.NoteSpan
import dev.aperte.notav.core.NoteStyle
import androidx.core.net.toUri
import dev.aperte.notav.core.NoteLink
import dev.aperte.notav.core.noteEnter
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.KeyboardArrowDown
import androidx.compose.material.icons.filled.KeyboardArrowUp
import androidx.compose.material.icons.outlined.FindReplace
import androidx.compose.material.icons.outlined.Search
import androidx.compose.material3.IconToggleButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TextField
import androidx.compose.material3.TextFieldDefaults
import androidx.compose.runtime.SideEffect
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.width
import androidx.compose.ui.unit.sp
import androidx.compose.runtime.produceState
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import dev.aperte.notav.ntr
import kotlinx.coroutines.flow.MutableSharedFlow
import dev.aperte.notav.core.noteHeadings
import dev.aperte.notav.core.noteLinkAt
import androidx.compose.material.icons.automirrored.outlined.FormatListBulleted
import dev.aperte.notav.core.noteSpans
import dev.aperte.notav.core.noteTableTidy
import dev.aperte.notav.tr
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.FlowPreview
import kotlinx.coroutines.flow.debounce
import kotlinx.coroutines.flow.drop
import kotlinx.coroutines.withContext

/** How long typing pauses before the text is saved (and the sync asked for), as on Linux. */
private const val SAVE_AFTER = 800L

/** Longer notes are not styled: each change parses the whole note again, as on Linux. */
private const val MAX_STYLED = 300_000

/**
 * The open note: its title (renamed in place, as the file's name) and its text, saved as typed.
 * Editor mode styles the Markdown (the marks dimmed), Source mode shows it plain.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun NotePane(model: Main, note: Note, onBack: (() -> Unit)?, onSide: () -> Unit) {
    val source by model.sourceMode.collectAsStateWithLifecycle()
    val find = remember(note.id) { FindState() }
    Scaffold(
        topBar = {
            Column {
            TopAppBar(
                title = { key(note.id, note.title) { TitleField(model, note) } },
                navigationIcon = {
                    onBack?.let {
                        Tip(tr("Back")) {
                            IconButton(onClick = it) { Icon(Icons.AutoMirrored.Filled.ArrowBack, tr("Back")) }
                        }
                    }
                },
                actions = {
                    // The mode it switches to, as the Linux app's view switcher.
                    val label = if (source) tr("Editor Mode") else tr("Source Mode")
                    Tip(label) {
                        IconButton(onClick = { model.setSourceMode(!source) }) {
                            Icon(if (source) Icons.Outlined.TextFields else Icons.Outlined.Code, label)
                        }
                    }
                    Tip(tr("Find in Note")) {
                        IconButton(onClick = { find.open = !find.open }) { Icon(Icons.Outlined.Search, tr("Find in Note")) }
                    }
                    // The side panel, also a swipe away.
                    Tip(tr("Outline")) {
                        IconButton(onClick = onSide) { Icon(Icons.AutoMirrored.Outlined.FormatListBulleted, tr("Outline")) }
                    }
                },
            )
            if (find.open) FindBar(find)
            }
        },
    ) { padding ->
        key(note.id) { NoteText(model, note, source, find, Modifier.padding(padding)) }
    }
}

/** What the find bar asks the note for. */
private enum class FindCommand { Next, Previous, Replace, ReplaceAll }

/**
 * Find and replace in the open note, as the Linux app's find bar: what is typed is found (case
 * ignored) from the cursor on, the matches highlighted and counted; Enter and the arrows go
 * through them, Replace and Replace All change them.
 */
private class FindState {
    val query = TextFieldState()
    val replace = TextFieldState()
    var open by mutableStateOf(false)
    var replacing by mutableStateOf(false)
    /** How many matches there are, and which is the current one (-1: none). */
    var count by mutableIntStateOf(0)
    var at by mutableIntStateOf(-1)
    val commands = MutableSharedFlow<FindCommand>(extraBufferCapacity = 4)
}

/** Where `query` starts in `text`, case ignored. */
private fun matches(text: CharSequence, query: String): List<Int> {
    if (query.isEmpty()) return emptyList()
    val s = text.toString()
    val found = mutableListOf<Int>()
    var at = s.indexOf(query, 0, ignoreCase = true)
    while (at >= 0) {
        found += at
        at = s.indexOf(query, at + query.length, ignoreCase = true)
    }
    return found
}

@Composable
private fun FindBar(find: FindState) {
    BackHandler { find.open = false }
    val focus = remember { FocusRequester() }
    LaunchedEffect(Unit) { focus.requestFocus() }
    val field = @Composable { state: TextFieldState, hint: String, modifier: Modifier, onEnter: () -> Unit ->
        TextField(
            state,
            modifier,
            placeholder = { Text(hint) },
            lineLimits = TextFieldLineLimits.SingleLine,
            keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
            onKeyboardAction = { onEnter() },
            shape = MaterialTheme.shapes.extraLarge,
            colors = TextFieldDefaults.colors(
                focusedIndicatorColor = Color.Transparent,
                unfocusedIndicatorColor = Color.Transparent,
            ),
        )
    }
    Column(Modifier.padding(horizontal = 8.dp, vertical = 4.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            field(find.query, tr("Find in note"), Modifier.weight(1f).focusRequester(focus)) {
                find.commands.tryEmit(FindCommand.Next)
            }
            // "3 of 12" on a match, else how many there are.
            val count = when {
                find.query.text.isEmpty() -> ""
                find.at >= 0 -> tr("{at} of {count}").replace("{at}", "${find.at + 1}").replace("{count}", "${find.count}")
                else -> ntr("{count} match", "{count} matches", find.count.toLong()).replace("{count}", "${find.count}")
            }
            Text(
                count,
                Modifier.padding(horizontal = 8.dp),
                style = MaterialTheme.typography.labelLarge,
                color = if (find.count == 0 && count.isNotEmpty()) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Tip(tr("Previous Match")) {
                IconButton(onClick = { find.commands.tryEmit(FindCommand.Previous) }) { Icon(Icons.Filled.KeyboardArrowUp, tr("Previous Match")) }
            }
            Tip(tr("Next Match")) {
                IconButton(onClick = { find.commands.tryEmit(FindCommand.Next) }) { Icon(Icons.Filled.KeyboardArrowDown, tr("Next Match")) }
            }
            Tip(tr("Replace")) {
                IconToggleButton(find.replacing, { find.replacing = it }) { Icon(Icons.Outlined.FindReplace, tr("Replace")) }
            }
            Tip(tr("Close")) {
                IconButton(onClick = { find.open = false }) { Icon(Icons.Filled.Close, tr("Close")) }
            }
        }
        if (find.replacing) {
            Row(Modifier.padding(top = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                field(find.replace, tr("Replace with"), Modifier.weight(1f)) { find.commands.tryEmit(FindCommand.Replace) }
                TextButton(onClick = { find.commands.tryEmit(FindCommand.Replace) }) { Text(tr("Replace")) }
                TextButton(onClick = { find.commands.tryEmit(FindCommand.ReplaceAll) }) { Text(tr("Replace All")) }
            }
        }
    }
}

/** The note's title, its file name: renamed when Done is pressed or the field is left. */
@Composable
private fun TitleField(model: Main, note: Note) {
    val state = rememberTextFieldState(note.title)
    var focused by remember { mutableStateOf(false) }
    val rename = {
        val title = state.text.toString().trim()
        if (title.isEmpty()) state.setTextAndPlaceCursorAtEnd(note.title)
        else if (title != note.title) model.renameNote(note, title)
    }
    BasicTextField(
        state,
        Modifier.fillMaxWidth().onFocusChanged {
            if (focused && !it.isFocused) rename()
            focused = it.isFocused
        },
        textStyle = MaterialTheme.typography.titleLarge.copy(color = MaterialTheme.colorScheme.onSurface),
        lineLimits = TextFieldLineLimits.SingleLine,
        keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Sentences, imeAction = ImeAction.Done),
        onKeyboardAction = { rename() },
        cursorBrush = SolidColor(MaterialTheme.colorScheme.primary),
    )
}

@OptIn(FlowPreview::class)
@Composable
private fun NoteText(model: Main, note: Note, source: Boolean, find: FindState, modifier: Modifier) {
    val state = remember { TextFieldState() }
    // The text as last loaded or saved: what the note is in the core.
    var saved by remember { mutableStateOf<String?>(null) }
    val changes by model.notav.changes.collectAsStateWithLifecycle()
    // Loaded, then again when a sync brings another text while nothing is typed here.
    LaunchedEffect(changes) {
        val text = withContext(Dispatchers.IO) { model.core.noteText(note.id) } ?: return@LaunchedEffect
        val shown = saved
        if (shown == null || (text != shown && state.text.toString() == shown)) {
            saved = text
            if (shown == null) {
                state.edit { replace(0, length, text); selection = androidx.compose.ui.text.TextRange(0) }
            } else {
                val at = state.selection.start.coerceAtMost(text.length)
                state.edit { replace(0, length, text); selection = androidx.compose.ui.text.TextRange(at) }
            }
        }
    }
    // Saved once typing pauses, and when the note is left.
    val current by rememberUpdatedState(saved)
    LaunchedEffect(Unit) {
        snapshotFlow { state.text.toString() }.drop(1).debounce(SAVE_AFTER).collect { text ->
            if (current != null && text != current) {
                saved = text
                model.saveText(note.id, text)
            }
        }
    }
    DisposableEffect(Unit) {
        onDispose {
            val text = state.text.toString()
            if (current != null && text != current) model.saveText(note.id, text)
        }
    }
    if (saved == null) return

    // The outline's headings, as typed.
    LaunchedEffect(Unit) {
        snapshotFlow { state.text.toString() }.debounce(300).collect { t ->
            model.headings.value = withContext(Dispatchers.Default) { noteHeadings(t) }
        }
    }
    DisposableEffect(Unit) { onDispose { model.headings.value = emptyList() } }

    // Editor mode's font from Settings; Source mode's monospace.
    val editorFont by produceState<String?>(null, changes) {
        value = withContext(Dispatchers.IO) { model.core.setting("editor_font") }
    }
    val colors = MaterialTheme.colorScheme
    val styled = !source && state.text.length <= MAX_STYLED
    val spans = remember(styled) { SpanCache() }
    // Not a state: written on every layout, it would recompose the field, which lays it out again.
    val layout = remember { arrayOfNulls<TextLayoutResult>(1) }
    val scroll = rememberScrollState()
    // A place in the text brought into view, a third down the screen, unless it is in sight.
    val reveal: suspend (Int) -> Unit = reveal@{ at ->
        val l = layout[0] ?: return@reveal
        val line = l.getLineForOffset(at.coerceIn(0, state.text.length))
        val (top, bottom) = l.getLineTop(line).toInt() to l.getLineBottom(line).toInt()
        if (top < scroll.value || bottom > scroll.value + scroll.viewportSize) {
            scroll.animateScrollTo((top - scroll.viewportSize / 3).coerceAtLeast(0))
        }
    }
    // Find: the matches of what the find bar has, while it is open.
    val query = if (find.open) find.query.text.toString() else ""
    val found = remember(state.text.toString(), query) { matches(state.text, query) }
    SideEffect {
        find.count = found.size
        if (find.at >= found.size) find.at = found.size - 1
    }
    // Typed in the find bar: the first match from the cursor on.
    LaunchedEffect(query) {
        val f = matches(state.text, query)
        find.at = f.indexOfFirst { it >= state.selection.start }.takeIf { it >= 0 } ?: (if (f.isEmpty()) -1 else 0)
        if (find.at >= 0) reveal(f[find.at])
    }
    val foundNow by rememberUpdatedState(found)
    LaunchedEffect(Unit) {
        find.commands.collect { c ->
            val f = foundNow
            val q = find.query.text.toString()
            if (f.isEmpty()) return@collect
            val r = find.replace.text.toString()
            when (c) {
                FindCommand.Next -> find.at = (find.at + 1).mod(f.size)
                FindCommand.Previous -> find.at = (find.at.coerceAtLeast(0) - 1).mod(f.size)
                FindCommand.Replace -> {
                    val s = f[find.at.coerceAtLeast(0)]
                    state.edit { replace(s, s + q.length, r) }
                    // On to the next match after what was put in.
                    val after = matches(state.text, q)
                    find.at = after.indexOfFirst { it >= s + r.length }.takeIf { it >= 0 } ?: (if (after.isEmpty()) -1 else 0)
                    if (find.at >= 0) reveal(after[find.at])
                    return@collect
                }
                FindCommand.ReplaceAll -> {
                    state.edit { for (s in f.asReversed()) replace(s, s + q.length, r) }
                    find.at = -1
                    return@collect
                }
            }
            reveal(f[find.at])
        }
    }
    // A heading picked in the outline: the cursor there, its line at the top.
    LaunchedEffect(Unit) {
        model.jump.collect { to ->
            val at = to.coerceIn(0, state.text.length)
            state.edit { selection = androidx.compose.ui.text.TextRange(at) }
            val l = layout[0] ?: return@collect
            scroll.animateScrollTo(l.getLineTop(l.getLineForOffset(at)).toInt())
        }
    }
    // The characters styled in full: all of a short note, else around what is on screen, moved
    // as it scrolls past (see `MarkdownStyles`).
    var window by remember { mutableStateOf(0..Int.MAX_VALUE) }
    if (styled) {
        val density = androidx.compose.ui.platform.LocalDensity.current
        LaunchedEffect(Unit) {
            snapshotFlow { Triple(scroll.value, scroll.viewportSize, state.text.length) }.collect { (top, height, length) ->
                val l = layout[0]
                if (length <= STYLED_WHOLE || l == null) {
                    window = 0..Int.MAX_VALUE
                    return@collect
                }
                val pad = with(density) { 8.dp.toPx() }
                val from = l.getOffsetForPosition(Offset(0f, top - pad))
                val to = l.getOffsetForPosition(Offset(l.size.width.toFloat(), top - pad + height))
                // Only once the screen nears the window's edge: each move lays the note out again.
                if (from - MARGIN / 2 < window.first || to + MARGIN / 2 > window.last) {
                    window = (from - MARGIN).coerceAtLeast(0)..(to + MARGIN)
                }
            }
        }
    }
    // A table is aligned once the cursor leaves the row it was changed in (the synced
    // `format_tables`, on unless "false"), as on Linux.
    if (styled) {
        LaunchedEffect(Unit) {
            if (withContext(Dispatchers.IO) { model.core.setting("format_tables") } == "false") return@LaunchedEffect
            var last = state.text.toString()
            var edited: Int? = null
            snapshotFlow { state.text.toString() to state.selection }.collect { (text, selection) ->
                if (text != last) {
                    last = text
                    edited = selection.start
                    return@collect
                }
                val at = edited ?: return@collect
                if (!selection.collapsed || lineStart(text, at) == lineStart(text, selection.start)) return@collect
                edited = null
                val c = noteTableTidy(text, at.toUInt(), selection.start.toUInt()) ?: return@collect
                state.edit {
                    replace(c.start.toInt(), c.end.toInt(), c.text)
                    this.selection = androidx.compose.ui.text.TextRange(c.cursor.toInt())
                }
                last = state.text.toString()
            }
        }
    }
    // Being typed in: focused with the keyboard up (Back hides the keyboard, the focus stays).
    var focused by remember { mutableStateOf(false) }
    @OptIn(ExperimentalLayoutApi::class)
    val keyboard = WindowInsets.isImeVisible
    val editing by rememberUpdatedState(focused && keyboard)
    val context = androidx.compose.ui.platform.LocalContext.current
    val open: (NoteLink) -> Unit = { link ->
        when (link) {
            is NoteLink.Address -> runCatching {
                context.startActivity(android.content.Intent(android.content.Intent.ACTION_VIEW, link.url.toUri()))
            }
            is NoteLink.Note -> model.followLink(note, link.target, link.wiki)
        }
    }
    val textStyle = MaterialTheme.typography.bodyLarge.copy(
        color = colors.onSurface,
        fontFamily = when {
            source || editorFont == "Monospace" -> FontFamily.Monospace
            editorFont == "Serif" -> FontFamily.Serif
            else -> null
        },
    )
    // Tables and code blocks never wrap: each scrolls sideways on its own. Out of the cursor,
    // the field keeps their text shrunk to nothing (each line one line, the positions as they
    // are) and a strip over it shows them; with the cursor in one, it is the field's to edit.
    val fieldFocus = remember { FocusRequester() }
    val selection = state.selection
    val collapsed = if (!styled) {
        emptyList()
    } else {
        spans.of(state.text)
            .filter { it.style == NoteStyle.CodeBlock || it.style == NoteStyle.Table }
            .map { it.start.toInt() until it.end.toInt().coerceAtMost(state.text.length) }
            .filter { !(focused && selection.start in it.first..it.last + 1) }
    }
    // Where each strip goes, from the field's layout: written only when it changes, and read
    // by the strips alone (the field laid out again for each write would never settle).
    var strips by remember { mutableStateOf(emptyList<Strip>()) }
    val collapsedNow by rememberUpdatedState(collapsed)
    BasicTextField(
        state,
        modifier.fillMaxSize().imePadding().verticalScroll(scroll)
            .then(
                if (styled) {
                    Modifier.tickTasks(state, spans, { editing }, open, { at -> collapsedNow.any { at in it } }) { layout[0] }
                } else {
                    Modifier
                },
            )
            .padding(horizontal = 16.dp, vertical = 8.dp)
            .then(if (styled) Modifier.blocks(state, spans, colors) { layout[0] } else Modifier)
            .focusRequester(fieldFocus)
            .onFocusChanged { focused = it.isFocused },
        textStyle = textStyle,
        keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Sentences),
        inputTransformation = if (styled) ContinueLists else null,
        outputTransformation = if (styled || found.isNotEmpty()) {
            MarkdownStyles(spans, colors, window, styled, found, query.length, find.at, collapsed)
        } else {
            null
        },
        onTextLayout = {
            val l = it()
            layout[0] = l
            if (l != null) {
                val s = strips(state.text, collapsedNow, l)
                if (s != strips) strips = s
            }
        },
        cursorBrush = SolidColor(colors.primary),
        decorator = { field ->
            Box {
                field()
                if (styled) {
                    Strips(
                        { strips }, state, spans, colors, textStyle.copy(fontFamily = FontFamily.Monospace),
                        found, query.length, found.getOrNull(find.at),
                    ) { at ->
                        // A tap in one: the cursor there, the block the field's to edit.
                        state.edit { this.selection = androidx.compose.ui.text.TextRange(at) }
                        fieldFocus.requestFocus()
                    }
                }
            }
        },
    )
}

/** A table or a code block shown over the field: where its lines start in the text, and their
 * tops in the field's layout. */
private data class Strip(val lines: List<Int>, val tops: List<Float>, val bottom: Float, val width: Int)

/** Where the strips of the `collapsed` blocks go in layout `l`. */
private fun strips(text: CharSequence, collapsed: List<IntRange>, l: TextLayoutResult): List<Strip> = collapsed.mapNotNull { r ->
    if (r.isEmpty() || r.last >= text.length) return@mapNotNull null
    val lines = buildList {
        add(r.first)
        for (i in r) if (text[i] == '\n' && i + 1 <= r.last) add(i + 1)
    }
    val tops = lines.map { l.getLineTop(l.getLineForOffset(it)) }
    val last = l.getLineForOffset(r.last.coerceAtLeast(r.first))
    // The field's width, not the text's: with the blocks shrunk its widest line may be short.
    val c = l.layoutInput.constraints
    Strip(lines, tops, l.getLineBottom(last), if (c.hasBoundedWidth) c.maxWidth else l.size.width)
}

/**
 * The strips: each block's lines with Editor mode's styles, unwrapped, at their lines' places,
 * scrolled sideways on their own. A tap gives `onTap` where in the text it was.
 */
@Composable
private fun Strips(
    strips: () -> List<Strip>,
    state: TextFieldState,
    spans: SpanCache,
    colors: ColorScheme,
    style: androidx.compose.ui.text.TextStyle,
    /** The find bar's matches, highlighted as in the field; the strip with the current one
     * scrolls sideways to it, as a strip follows the cursor on Linux. */
    found: List<Int>,
    matchLength: Int,
    current: Int?,
    onTap: (Int) -> Unit,
) {
    val density = androidx.compose.ui.platform.LocalDensity.current
    // Each keeps its own sideways scroll, by where it starts.
    for (s in strips()) key(s.lines.first()) {
        val top = s.tops.first()
        val text = state.text
        val all = spans.of(text)
        // Each line with the styles of the spans over it, as the field would show it.
        val lines = s.lines.map { start ->
            val end = text.indexOf('\n', start).let { if (it < 0) text.length else it }
            androidx.compose.ui.text.buildAnnotatedString {
                append(text.substring(start, end))
                for (span in all) {
                    val (a, b) = span.start.toInt().coerceAtLeast(start) to span.end.toInt().coerceAtMost(end)
                    if (a >= b) continue
                    look(span.style, colors)?.let { addStyle(it, a - start, b - start) }
                }
                for (f in found) {
                    val (a, b) = f.coerceAtLeast(start) to (f + matchLength).coerceAtMost(end)
                    if (a >= b) continue
                    addStyle(
                        if (f == current) {
                            SpanStyle(background = colors.tertiary, color = colors.onTertiary)
                        } else {
                            SpanStyle(background = colors.tertiaryContainer, color = colors.onTertiaryContainer)
                        },
                        a - start,
                        b - start,
                    )
                }
            }
        }
        val layouts = remember(s) { arrayOfNulls<TextLayoutResult>(s.lines.size) }
        val sideways = rememberScrollState()
        // The current match in this strip: in sight, a third in from the left.
        LaunchedEffect(current, s) {
            val at = current ?: return@LaunchedEffect
            // Its line in the strip, if it is in one of them.
            val i = s.lines.indexOfLast { it <= at }
            if (i < 0 || at > text.indexOf('\n', s.lines[i]).let { if (it < 0) text.length else it }) return@LaunchedEffect
            val x = layouts[i]?.getHorizontalPosition(at - s.lines[i], true) ?: return@LaunchedEffect
            if (x < sideways.value || x > sideways.value + s.width) sideways.animateScrollTo((x - s.width / 3).toInt().coerceAtLeast(0))
        }
        androidx.compose.foundation.layout.Box(
            Modifier.offset { androidx.compose.ui.unit.IntOffset(0, top.toInt()) }
                .width(with(density) { s.width.toDp() })
                .height(with(density) { (s.bottom - top).toDp() })
                // A shade at each edge while there is more of the block that way, as on Linux:
                // a block not shown whole is seen to be one at a glance.
                .drawWithContent {
                    drawContent()
                    val edge = 16.dp.toPx()
                    val shade = colors.onSurface.copy(alpha = 0.15f)
                    if (sideways.canScrollBackward) {
                        drawRect(Brush.horizontalGradient(listOf(shade, Color.Transparent), 0f, edge), size = Size(edge, size.height))
                    }
                    if (sideways.canScrollForward) {
                        drawRect(
                            Brush.horizontalGradient(listOf(Color.Transparent, shade), size.width - edge, size.width),
                            topLeft = Offset(size.width - edge, 0f),
                            size = Size(edge, size.height),
                        )
                    }
                }
                .horizontalScroll(sideways),
        ) {
            androidx.compose.ui.layout.Layout(
                content = { lines.forEachIndexed { i, t -> Text(t, style = style, softWrap = false, maxLines = 1, onTextLayout = { layouts[i] = it }) } },
                modifier = Modifier.pointerInput(s) {
                    detectTapGestures { p ->
                        // The line under the tap, then the character.
                        val i = s.tops.indexOfLast { it - top <= p.y }.coerceAtLeast(0)
                        val col = layouts[i]?.getOffsetForPosition(Offset(p.x, 0f)) ?: 0
                        onTap(s.lines[i] + col)
                    }
                },
            ) { measurables, constraints ->
                val placeables = measurables.map { it.measure(androidx.compose.ui.unit.Constraints()) }
                val width = maxOf(placeables.maxOfOrNull { it.width } ?: 0, constraints.minWidth)
                layout(width, (s.bottom - top).toInt()) {
                    placeables.forEachIndexed { i, p -> p.place(0, (s.tops[i] - top).toInt()) }
                }
            }
        }
    }
}

/** The spans of the text last asked for: parsed once per text, for the styles and the boxes. */
private class SpanCache {
    private var text: CharSequence? = null
    private var spans: List<NoteSpan> = emptyList()

    fun of(t: CharSequence): List<NoteSpan> {
        // By content: the field hands over its own CharSequence, never equal to a String.
        val last = text
        if (last == null || !last.contentEquals(t)) {
            val s = t.toString()
            spans = noteSpans(s)
            text = s
        }
        return spans
    }
}

/** Editor mode's look of a range, as the Linux app's style scheme (`sourceview/notav.xml`) in
 * the theme's colours. Block backgrounds are drawn behind (`blocks`). */
private fun look(style: NoteStyle, c: ColorScheme): SpanStyle? = when (style) {
    is NoteStyle.Heading -> SpanStyle(
        fontWeight = FontWeight.Bold,
        fontSize = when (style.level.toInt()) {
            1 -> 1.5.em
            2 -> 1.3.em
            3 -> 1.15.em
            4 -> 1.05.em
            else -> 1.em
        },
    )
    NoteStyle.Strong, NoteStyle.TableHeader -> SpanStyle(fontWeight = FontWeight.Bold)
    NoteStyle.Emphasis -> SpanStyle(fontStyle = FontStyle.Italic)
    NoteStyle.Strikethrough -> SpanStyle(textDecoration = TextDecoration.LineThrough)
    NoteStyle.Code -> SpanStyle(fontFamily = FontFamily.Monospace, background = c.surfaceContainerHighest)
    NoteStyle.CodeBlock, NoteStyle.Table -> SpanStyle(fontFamily = FontFamily.Monospace)
    NoteStyle.Quote -> SpanStyle(color = c.onSurfaceVariant)
    is NoteStyle.Alert -> null
    is NoteStyle.AlertMark -> SpanStyle(color = alertColor(style.kind, c), fontWeight = FontWeight.Bold)
    NoteStyle.Link -> SpanStyle(color = c.primary, textDecoration = TextDecoration.Underline)
    NoteStyle.ListMark -> SpanStyle(color = c.onSurfaceVariant, fontWeight = FontWeight.Bold)
    is NoteStyle.Task -> SpanStyle(color = c.primary, fontWeight = FontWeight.Bold)
    NoteStyle.Mark -> SpanStyle(color = c.onSurfaceVariant.copy(alpha = 0.6f))
}

/** GitHub's alerts in their colours, readable on the theme's light or dark surface. */
private fun alertColor(kind: Alert, c: ColorScheme): Color {
    val dark = c.surface.luminance() < 0.5f
    return when (kind) {
        Alert.NOTE -> if (dark) Color(0xFF78AEED) else Color(0xFF1C71D8)
        Alert.TIP -> if (dark) Color(0xFF6BC495) else Color(0xFF1B8553)
        Alert.IMPORTANT -> if (dark) Color(0xFFC88FE0) else Color(0xFF9141AC)
        Alert.WARNING -> if (dark) Color(0xFFE5B850) else Color(0xFF9C6E03)
        Alert.CAUTION -> if (dark) Color(0xFFF08A90) else Color(0xFFC01C28)
    }
}

private fun Color.luminance() = 0.2126f * red + 0.7152f * green + 0.0722f * blue

/**
 * The core's spans as styles of the text shown; the dimmed marks last, over the rest. Android
 * adds each style in time growing with those already there, so a long note (thousands of
 * them) is styled in full only in `window`, around what is on screen; what changes the lines'
 * heights and widths (headings, code, tables) everywhere, so scrolling does not jump.
 */
private class MarkdownStyles(
    val spans: SpanCache,
    val colors: ColorScheme,
    val window: IntRange,
    /** Editor mode's styles; else (Source mode, a long note) only what is found. */
    val styled: Boolean,
    /** The find bar's matches (where each starts, `matchLength` long), `current` the one it is on. */
    val found: List<Int>,
    val matchLength: Int,
    val current: Int,
    /** Tables and code blocks shown by their strips: here shrunk to nothing, one line each. */
    val collapsed: List<IntRange> = emptyList(),
) : OutputTransformation {
    override fun TextFieldBuffer.transformOutput() {
        if (styled) markdown()
        for (r in collapsed) {
            if (r.isEmpty() || r.last >= length) continue
            addStyle(COLLAPSED, r.first, r.last + 1)
        }
        found.forEachIndexed { i, s ->
            val end = s + matchLength
            if (end < window.first || s > window.last || end > length) return@forEachIndexed
            addStyle(
                if (i == current) {
                    SpanStyle(background = colors.tertiary, color = colors.onTertiary)
                } else {
                    SpanStyle(background = colors.tertiaryContainer, color = colors.onTertiaryContainer)
                },
                s,
                end,
            )
        }
    }

    private fun TextFieldBuffer.markdown() {
        val all = spans.of(asCharSequence())
        for (pass in 0..1) {
            for (s in all) {
                val mark = s.style == NoteStyle.Mark || s.style is NoteStyle.AlertMark
                if (mark != (pass == 1)) continue
                val (start, end) = s.start.toInt() to s.end.toInt().coerceAtMost(length)
                val shapes = s.style is NoteStyle.Heading || s.style == NoteStyle.CodeBlock || s.style == NoteStyle.Table
                if (!shapes && (end < window.first || start > window.last)) continue
                val look = look(s.style, colors) ?: continue
                if (start < end) addStyle(look, start, end)
            }
        }
    }

    override fun equals(other: Any?) =
        other is MarkdownStyles && other.spans === spans && other.colors == colors && other.window == window &&
            other.styled == styled && other.found == found && other.matchLength == matchLength && other.current == current &&
            other.collapsed == collapsed
    override fun hashCode() = (spans.hashCode() * 31 + window.hashCode()) * 31 + found.hashCode() + current
}

/** A collapsed block's text: too small to wrap (a few hundred characters fit in a line's
 * width), not seen; its lines keep the text's line height. */
private val COLLAPSED = SpanStyle(fontSize = 0.5.sp, color = Color.Transparent, background = Color.Transparent)

/** Notes up to this long are styled whole: a few hundred styles, laid out at once. */
private const val STYLED_WHOLE = 10_000

/** How far around the screen a longer note is styled, in characters. */
private const val MARGIN = 4_000

/** Enter in a list item or a quote continues it, or ends it on an empty one (`noteEnter`). */
private object ContinueLists : InputTransformation {
    @OptIn(androidx.compose.foundation.ExperimentalFoundationApi::class)
    override fun TextFieldBuffer.transformInput() {
        if (changes.changeCount != 1) return
        val range = changes.getRange(0)
        val original = changes.getOriginalRange(0)
        if (!original.collapsed || range.length != 1 || asCharSequence()[range.start] != '\n') return
        val before = originalText.toString()
        val edits = noteEnter(before, original.start.toUInt())
        if (edits.isEmpty()) return
        revertAllChanges()
        for (e in edits.asReversed()) replace(e.start.toInt(), e.end.toInt(), e.text)
        val first = edits.first()
        selection = androidx.compose.ui.text.TextRange(first.start.toInt() + first.text.length)
    }
}

/** Where the line holding `at` starts. */
private fun lineStart(text: String, at: Int) = text.lastIndexOf('\n', (at - 1).coerceAtLeast(0)) + 1

/**
 * A tap on a task's box ticks or unticks it, before the field would move the cursor there, as a
 * click does on Linux. While the note is not being edited (`editing`), a tap on a link opens
 * it (`open`), as Todav's task notes do; editing, it places the cursor.
 */
private fun Modifier.tickTasks(
    state: TextFieldState,
    spans: SpanCache,
    editing: () -> Boolean,
    open: (NoteLink) -> Unit,
    /** Whether a place is in a block its strip shows (the strip takes the taps there). */
    stripped: (Int) -> Boolean,
    layout: () -> TextLayoutResult?,
) =
    pointerInput(state) {
        awaitEachGesture {
            val down = awaitFirstDown(pass = PointerEventPass.Initial)
            val l = layout() ?: return@awaitEachGesture
            // The field's text starts 16dp in and 8dp down (its padding).
            val at = l.getOffsetForPosition(down.position - Offset(16.dp.toPx(), 8.dp.toPx()))
            if (stripped(at)) return@awaitEachGesture
            val all = spans.of(state.text)
            val link = all.firstOrNull { it.style == NoteStyle.Link && at >= it.start.toInt() && at < it.end.toInt() }
            if (link != null && !editing()) {
                val target = noteLinkAt(state.text.toString(), at.toUInt()) ?: return@awaitEachGesture
                down.consume()
                val up = waitForUpOrCancellation(PointerEventPass.Initial) ?: return@awaitEachGesture
                up.consume()
                open(target)
                return@awaitEachGesture
            }
            val box = all.firstOrNull {
                it.style is NoteStyle.Task && at >= it.start.toInt() && at < it.end.toInt()
            } ?: return@awaitEachGesture
            val bounds = l.getBoundingBox(box.start.toInt())
            if (down.position.x - 16.dp.toPx() > l.getBoundingBox(box.end.toInt() - 1).right + 8.dp.toPx() ||
                down.position.x - 16.dp.toPx() < bounds.left - 8.dp.toPx()
            ) {
                return@awaitEachGesture
            }
            down.consume()
            val up = waitForUpOrCancellation(PointerEventPass.Initial) ?: return@awaitEachGesture
            up.consume()
            val done = (box.style as NoteStyle.Task).done
            state.edit { replace(box.start.toInt() + 1, box.start.toInt() + 2, if (done) " " else "x") }
        }
    }

/** Code blocks, quotes and alerts drawn as boxes behind their lines. */
private fun Modifier.blocks(state: TextFieldState, spans: SpanCache, c: ColorScheme, layout: () -> TextLayoutResult?) =
    drawBehind {
        val l = layout() ?: return@drawBehind
        val text = state.text
        // Alerts last, see-through: their box goes on over a code block in them, as on Linux.
        for (s in spans.of(text).sortedBy { it.style is NoteStyle.Alert }) {
            val color = when (val style = s.style) {
                NoteStyle.CodeBlock -> c.surfaceContainerHigh
                is NoteStyle.Alert -> alertColor(style.kind, c).copy(alpha = 0.12f)
                NoteStyle.Quote -> c.surfaceContainer
                else -> continue
            }
            val (start, end) = s.start.toInt() to (s.end.toInt() - 1).coerceIn(s.start.toInt(), text.length)
            if (start >= text.length && text.isNotEmpty()) continue
            val top = l.getLineTop(l.getLineForOffset(start))
            val bottom = l.getLineBottom(l.getLineForOffset(end))
            drawRoundRect(
                color,
                topLeft = Offset(-8.dp.toPx(), top - 2.dp.toPx()),
                size = Size(size.width + 16.dp.toPx(), bottom - top + 4.dp.toPx()),
                cornerRadius = CornerRadius(8.dp.toPx()),
            )
        }
    }
