package dev.aperte.notav.ui

import android.content.Intent
import android.net.Uri
import android.os.Build
import android.provider.Settings as SystemSettings
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.add
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.outlined.Login
import androidx.compose.material.icons.outlined.AccountCircle
import androidx.compose.material.icons.outlined.Add
import androidx.compose.material.icons.outlined.Badge
import androidx.compose.material.icons.outlined.Check
import androidx.compose.material.icons.outlined.CheckCircle
import androidx.compose.material.icons.outlined.Cloud
import androidx.compose.material.icons.outlined.Delete
import androidx.compose.material.icons.outlined.EditNote
import androidx.compose.material.icons.outlined.ErrorOutline
import androidx.compose.material.icons.outlined.FavoriteBorder
import androidx.compose.material.icons.outlined.Folder
import androidx.compose.material.icons.outlined.Info
import androidx.compose.material.icons.outlined.LocalCafe
import androidx.compose.material.icons.outlined.Palette
import androidx.compose.material.icons.outlined.PhoneAndroid
import androidx.compose.material.icons.outlined.Today
import androidx.compose.material.icons.outlined.VolunteerActivism
import androidx.compose.material.icons.outlined.WarningAmber
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.ExposedDropdownMenu
import androidx.compose.material3.ExposedDropdownMenuAnchorType
import androidx.compose.material3.ExposedDropdownMenuBox
import androidx.compose.material3.ExposedDropdownMenuDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LargeFlexibleTopAppBar
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.MenuDefaults
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SelectableDropdownMenuItem
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.key
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.foundation.gestures.detectVerticalDragGestures
import androidx.compose.material.icons.filled.DragHandle
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import androidx.core.net.toUri
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import dev.aperte.notav.Main
import dev.aperte.notav.Page
import dev.aperte.notav.SyncState
import dev.aperte.notav.Upload
import dev.aperte.notav.accountName
import dev.aperte.notav.core.Account
import dev.aperte.notav.core.AccountKind
import dev.aperte.notav.ntr
import dev.aperte.notav.ptr
import dev.aperte.notav.reason
import dev.aperte.notav.tr
import java.text.DateFormat
import java.time.LocalDate
import java.util.Date
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/**
 * A page over the notes: Settings and its pages, as Todav's (the Linux app's Preferences, a
 * page each), an account's page, adding one and signing in again. Back goes to its parent.
 */
@Composable
fun PageView(model: Main, page: Page) {
    val back = {
        model.cancelSignIn()
        model.page.value = page.parent
    }
    BackHandler(onBack = back)
    when (page) {
        Page.Settings -> Frame(tr("Settings"), back) { SettingsMenu(model) }
        Page.General -> Frame(tr("General"), back) { General(model) }
        Page.Editor -> Frame(tr("Editor"), back) { EditorSettings(model) }
        Page.Journal -> Frame(tr("Journal"), back) { Journal(model) }
        Page.Accounts -> Frame(tr("Accounts"), back) { Accounts(model) }
        Page.About -> Frame(tr("About Notav"), back) { About() }
        is Page.Account -> AccountPage(model, page.id, back)
        Page.Trash -> TrashPage(model, back)
        is Page.Versions -> VersionsPage(model, page.id, back)
        Page.AddAccount -> Frame(tr("Add Account"), back) {
            Steps { AccountChoice(model, welcome = false) }
        }
        is Page.Reconnect -> Frame(tr("Sign In Again"), back) {
            Steps {
                val account = model.screen.value?.accounts?.firstOrNull { it.id == page.id }
                var server by rememberSaveable { mutableStateOf(account?.url.orEmpty()) }
                account?.let { Text(accountName(it), color = MaterialTheme.colorScheme.onSurfaceVariant) }
                SignInForm(model, server) { server = it }
            }
        }
    }
}

/** A settings page, Todav's: its title in a large bar with a back button, its sections under it, centred on wide screens. */
@OptIn(ExperimentalMaterial3Api::class, ExperimentalMaterial3ExpressiveApi::class)
@Composable
internal fun Frame(
    title: String,
    onBack: () -> Unit,
    actions: @Composable androidx.compose.foundation.layout.RowScope.() -> Unit = {},
    content: @Composable ColumnScope.() -> Unit,
) {
    val bar = TopAppBarDefaults.exitUntilCollapsedScrollBehavior()
    BoxWithConstraints {
        // The bar's back button and title over the content, centred on wide screens; its colour
        // still across the whole width.
        val side = ((maxWidth - SETTINGS_WIDTH) / 2).coerceAtLeast(0.dp)
        Scaffold(
            modifier = Modifier.nestedScroll(bar.nestedScrollConnection),
            topBar = {
                LargeFlexibleTopAppBar(
                    title = { Text(title) },
                    navigationIcon = {
                        Tip(tr("Back")) {
                            IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Filled.ArrowBack, tr("Back")) }
                        }
                    },
                    actions = actions,
                    windowInsets = TopAppBarDefaults.windowInsets.add(WindowInsets(left = side, right = side)),
                    scrollBehavior = bar,
                )
            },
        ) { padding ->
            Column(
                Modifier.fillMaxSize().padding(padding).imePadding().verticalScroll(rememberScrollState()),
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                Column(
                    Modifier.widthIn(max = SETTINGS_WIDTH).fillMaxWidth().navigationBarsPadding().padding(bottom = 24.dp),
                    content = content,
                )
            }
        }
    }
}

/** Editor mode's fonts to choose from, as `editor_font` keeps them; empty, the default. */
private val EDITOR_FONTS = listOf("", "Serif", "Monospace")

/** The widest the settings' rows get, centred. */
private val SETTINGS_WIDTH = 720.dp

/** Adding an account and signing in: the welcome page's steps, spaced as there. */
@Composable
private fun Steps(content: @Composable ColumnScope.() -> Unit) = Column(
    Modifier.padding(16.dp),
    verticalArrangement = Arrangement.spacedBy(12.dp),
    content = content,
)

/** A [Group] of rows under its title (none: only a gap), as Todav's. */
@Composable
internal fun Section(title: String?, content: @Composable ColumnScope.() -> Unit) {
    if (title != null) {
        Text(
            title,
            style = MaterialTheme.typography.titleSmall,
            color = MaterialTheme.colorScheme.primary,
            modifier = Modifier.padding(start = 32.dp, top = 24.dp, bottom = 8.dp),
        )
    } else {
        Spacer(Modifier.height(16.dp))
    }
    Group(Modifier.padding(horizontal = 16.dp), content)
}

/** A note under a [Section], as the Linux app's groups' descriptions. */
@Composable
internal fun Note(text: String) = Text(
    text,
    style = MaterialTheme.typography.bodyMedium,
    color = MaterialTheme.colorScheme.onSurfaceVariant,
    modifier = Modifier.padding(start = 32.dp, end = 32.dp, top = 12.dp),
)

/** What `read` gives (from the core, off the main thread), read again on every change. */
@Composable
private fun <T> load(model: Main, vararg keys: Any?, read: () -> T): T {
    val changes by model.notav.changes.collectAsStateWithLifecycle()
    val initial = remember(*keys) { read() }
    return produceState(initial, changes, *keys) { value = withContext(Dispatchers.IO) { read() } }.value
}

/** A setting kept in the core (synced unless device-only). */
@Composable
private fun setting(model: Main, key: String): String? = load(model, key) { model.core.setting(key) }

/** An on/off setting, on unless it says "false" (or off unless "true", with `default` false). */
@Composable
private fun SwitchRow(model: Main, title: String, key: String, subtitle: String? = null, default: Boolean = true) {
    val value = setting(model, key)
    val on = if (default) value != "false" else value == "true"
    Item(
        headlineContent = { Text(title) },
        supportingContent = subtitle?.let { { Text(it) } },
        trailingContent = { Switch(on, { model.setSetting(key, it.toString()) }) },
        modifier = Modifier.clickable { model.setSetting(key, (!on).toString()) },
    )
}

/** One of `options` (stored as `values`; unset, `default`) with connected buttons under `title`. */
@Composable
private fun ButtonsRow(model: Main, title: String, key: String, values: List<String>, options: List<String>, default: String = values[0]) {
    val current = values.indexOf(setting(model, key) ?: default).coerceAtLeast(values.indexOf(default))
    Item(
        headlineContent = { Text(title) },
        supportingContent = {
            Connected(options, current, Modifier.fillMaxWidth().padding(top = 8.dp)) { model.setSetting(key, values[it]) }
        },
    )
}

/**
 * A row naming the one of `options` picked (`selected`) under `title`, with `subtitle`; a tap
 * opens them in a menu below it (Material's select, an exposed dropdown menu).
 */
@OptIn(ExperimentalMaterial3Api::class, ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun Select(title: String, subtitle: String?, options: List<String>, selected: Int, onPick: (Int) -> Unit) {
    var open by remember { mutableStateOf(false) }
    ExposedDropdownMenuBox(open, { open = it }) {
        Item(
            headlineContent = { Text(title) },
            supportingContent = {
                Column {
                    Text(options.getOrElse(selected) { "" }, color = MaterialTheme.colorScheme.primary)
                    subtitle?.let { Text(it) }
                }
            },
            trailingContent = { ExposedDropdownMenuDefaults.TrailingIcon(open) },
            modifier = Modifier.menuAnchor(ExposedDropdownMenuAnchorType.PrimaryNotEditable),
        )
        ExposedDropdownMenu(expanded = open, onDismissRequest = { open = false }) {
            options.forEachIndexed { i, o ->
                SelectableDropdownMenuItem(
                    selected = i == selected,
                    onClick = {
                        open = false
                        onPick(i)
                    },
                    text = { Text(o) },
                    shapes = MenuDefaults.itemShape(i, options.size),
                    selectedLeadingIcon = { Icon(Icons.Outlined.Check, null) },
                )
            }
        }
    }
}

/**
 * A text to set, as the Linux app's entry rows with an apply button: `value` until typed over,
 * then applied with Done or the check button. Another `reset` shows `value` again. As it is
 * typed in, those of `suggestions` with its text in them show under it, one picked put in.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun ApplyField(
    value: String,
    label: String,
    icon: ImageVector? = null,
    reset: Int = 0,
    suggestions: () -> List<String> = { emptyList() },
    onApply: (String) -> Unit,
) {
    var text by rememberSaveable(value, reset) { mutableStateOf(value) }
    val changed = text != value
    var open by remember { mutableStateOf(false) }
    val shown = remember(text, open) {
        if (!open) emptyList() else suggestions().filter { it.contains(text, ignoreCase = true) && it != text }.take(8)
    }
    ExposedDropdownMenuBox(open && shown.isNotEmpty(), { open = it }) {
        FieldItem(
            text,
            {
                text = it
                open = true
            },
            label,
            Modifier.menuAnchor(ExposedDropdownMenuAnchorType.PrimaryEditable),
            icon = icon,
            singleLine = true,
            trailing = if (changed) {
                { Tip(tr("Apply")) { IconButton(onClick = { onApply(text) }) { Icon(Icons.Outlined.Check, tr("Apply")) } } }
            } else {
                null
            },
            keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
            keyboardActions = KeyboardActions(onDone = {
                open = false
                if (changed) onApply(text)
            }),
        )
        ExposedDropdownMenu(expanded = open && shown.isNotEmpty(), onDismissRequest = { open = false }) {
            shown.forEachIndexed { i, s ->
                androidx.compose.material3.DropdownMenuItem(
                    onClick = {
                        open = false
                        text = s
                    },
                    text = { Text(s) },
                    shape = MenuDefaults.itemShape(i, shown.size).shape,
                )
            }
        }
    }
}

// --- pages ------------------------------------------------------------------------------------

/** The settings' first page: its pages, as Android's own settings list them. */
@Composable
private fun SettingsMenu(model: Main) {
    Section(null) {
        listOf(
            Triple(Page.General, tr("General"), Icons.Outlined.Palette),
            Triple(Page.Editor, tr("Editor"), Icons.Outlined.EditNote),
            Triple(Page.Journal, tr("Journal"), Icons.Outlined.Today),
            Triple(Page.Accounts, tr("Accounts"), Icons.Outlined.AccountCircle),
            Triple(Page.About, tr("About Notav"), Icons.Outlined.Info),
        ).forEach { (page, title, icon) ->
            Item(
                headlineContent = { Text(title) },
                leadingContent = { Icon(icon, null) },
                modifier = Modifier.clickable { model.page.value = page },
            )
        }
    }
}

@Composable
private fun General(model: Main) {
    val context = LocalContext.current
    Section(tr("Appearance")) {
        ButtonsRow(
            model, tr("Style"), "color_scheme", listOf("system", "light", "dark"),
            listOf(ptr("style", "System"), ptr("style", "Light"), ptr("style", "Dark")),
        )
        SwitchRow(model, tr("Dense tree"), "dense_tree", tr("Smaller rows, as in Files"), default = false)
        ButtonsRow(
            model, tr("Icons in tree"), "tree_icons", listOf("all", "notes", "none"),
            listOf(ptr("tree icons", "All"), ptr("tree icons", "Notes Only"), ptr("tree icons", "None")),
            default = "notes",
        )
        // Android 13 and newer: each app's language in the system settings.
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            Item(
                headlineContent = { Text(tr("Language")) },
                supportingContent = { Text(tr("Notav’s language in the system settings")) },
                modifier = Modifier.clickable {
                    context.startActivity(
                        Intent(SystemSettings.ACTION_APP_LOCALE_SETTINGS, Uri.fromParts("package", context.packageName, null)),
                    )
                },
            )
        }
    }
    // Kept on this device only, as on Linux.
    Section(tr("Session")) {
        SwitchRow(model, tr("Restore session"), "restore_session", tr("Reopen the note left open when Notav starts"))
    }
}

@Composable
private fun EditorSettings(model: Main) {
    // Editor mode's font (the device-only `editor_font`, as on Linux, by the names Pango knows
    // too). Source mode's stays monospace: Android has one.
    Section(tr("Fonts")) {
        val current = EDITOR_FONTS.indexOf(setting(model, "editor_font").orEmpty()).coerceAtLeast(0)
        Select(
            tr("Editor mode"),
            null,
            listOf(ptr("font", "Default"), ptr("font", "Serif"), ptr("font", "Monospace")),
            current,
        ) { model.setSetting("editor_font", EDITOR_FONTS[it]) }
    }
    Section(tr("Markdown")) {
        SwitchRow(
            model, tr("Format tables"), "format_tables",
            tr("Keep a table’s columns lined up as you type and move between its rows and cells"),
        )
    }
}

/** The journal's daily notes, as the Linux app's Journal page: each field applied on its own. */
@Composable
private fun Journal(model: Main) {
    val core = model.core
    val folder = load(model) { core.journalFolder() }
    val template = setting(model, "journal_template").orEmpty()
    val format = load(model) { core.journalFormat() }
    val today = load(model) {
        val d = LocalDate.now()
        core.dailyPath(d.year, d.monthValue.toUInt(), d.dayOfMonth.toUInt()) + ".md"
    }
    Section(tr("Daily Notes")) {
        // Suggesting the folders and notes there are in the journal's account.
        ApplyField(
            folder, tr("Folder"), Icons.Outlined.Folder,
            suggestions = { core.journalAccount()?.let { core.folders(it) }.orEmpty() },
        ) { model.setJournalFolder(it) }
        ApplyField(
            template, tr("Template note"), Icons.Outlined.EditNote,
            suggestions = {
                val account = core.journalAccount()
                core.notes().filter { it.account == account }.map { it.path.removeSuffix(".md") }
            },
        ) { model.setJournalTemplate(it) }
    }
    Note(tr("A new daily note is a copy of the template note, with {{title}} and {{date}} in it filled in."))
    Section(tr("Note Name")) {
        ApplyField(format, tr("Format")) { model.setSetting("journal_format", it.trim()) }
        Item(headlineContent = { Text(tr("Today’s note")) }, supportingContent = { Text(today) })
    }
    Note(
        tr(
            "YYYY is the year, YY its last two digits, MM the month and DD the day; M and D are without a leading " +
                "zero. A / starts a folder, and text in [brackets] stays as written.",
        ),
    )
}

/** The accounts, each opening its page; settings sync and the background sync. */
@Composable
private fun Accounts(model: Main) {
    val accounts = load(model) { model.core.accounts() }
    // The order everything lists accounts in: dragged by the handles, as in Todav and the Linux
    // app.
    var order by remember(accounts) { mutableStateOf(accounts) }
    // The handles' gestures outlive recompositions: they read the order through this.
    val current by rememberUpdatedState(order)
    var held by remember { mutableStateOf<Long?>(null) }
    var shift by remember { mutableStateOf(0f) }
    Section(tr("Accounts")) {
        // Keyed by account, so a row keeps its handle's gesture while it moves.
        order.forEach { a ->
            key(a.id) {
                var height by remember { mutableStateOf(0) }
                Item(
                    headlineContent = { Text(accountName(a)) },
                    supportingContent = status(model, a)?.let { { Text(it) } },
                    leadingContent = { Icon(if (a.kind == AccountKind.LOCAL) Icons.Outlined.PhoneAndroid else Icons.Outlined.Cloud, null) },
                    trailingContent = if (order.size > 1) {
                        {
                            Icon(
                                Icons.Filled.DragHandle,
                                tr("Drag to Move"),
                                Modifier.pointerInput(a.id) {
                                    detectVerticalDragGestures(
                                        onDragStart = {
                                            held = a.id
                                            shift = 0f
                                        },
                                        onVerticalDrag = { change, dy ->
                                            change.consume()
                                            shift += dy
                                            // Past half a row (with the gap below it): one place up or down.
                                            val row = height + 2.dp.toPx()
                                            val i = current.indexOfFirst { it.id == a.id }
                                            val to = (i + Math.round(shift / row)).coerceIn(0, current.lastIndex)
                                            if (to != i) {
                                                order = current.toMutableList().apply { add(to, removeAt(i)) }
                                                shift -= (to - i) * row
                                            }
                                        },
                                        onDragEnd = {
                                            held = null
                                            model.reorderAccounts(current.map { it.id })
                                        },
                                        onDragCancel = {
                                            held = null
                                            order = accounts
                                        },
                                    )
                                },
                            )
                        }
                    } else {
                        null
                    },
                    containerColor = if (held == a.id) {
                        MaterialTheme.colorScheme.surfaceContainerHighest
                    } else {
                        MaterialTheme.colorScheme.surfaceContainer
                    },
                    modifier = Modifier.onSizeChanged { height = it.height }.clickable { model.page.value = Page.Account(a.id) },
                )
            }
        }
        Item(
            headlineContent = { Text(tr("Add Account")) },
            leadingContent = { Icon(Icons.Outlined.Add, null) },
            modifier = Modifier.clickable { model.page.value = Page.AddAccount },
        )
    }
    Section(tr("Sync")) {
        val servers = accounts.filter { it.kind == AccountKind.NEXTCLOUD }
        if (servers.isNotEmpty()) {
            val current = load(model) { model.core.settingsAccount() }
            Select(
                tr("Sync settings with"),
                tr("Accounts and servers always stay on this device"),
                listOf(ptr("settings sync", "Off")) + servers.map(::accountName),
                servers.indexOfFirst { it.id == current } + 1,
            ) { model.setSettingsAccount(servers.getOrNull(it - 1)?.id) }
        }
        val minutes = setting(model, "background_sync")?.toLongOrNull()?.takeIf { it in Upload.INTERVALS } ?: 0L
        Select(
            tr("Sync in the Background"),
            tr("While Notav is closed, to have other devices’ changes ready"),
            Upload.INTERVALS.map(::interval),
            Upload.INTERVALS.indexOf(minutes),
        ) { model.setBackgroundSync(Upload.INTERVALS[it]) }
    }
}

/** How often, as the background sync's choices name it. */
private fun interval(minutes: Long): String = when {
    minutes == 0L -> ptr("background sync", "Off")
    minutes == 60L -> tr("Every hour")
    minutes == 1440L -> tr("Once a day")
    minutes < 60 -> ntr("Every {n} minute", "Every {n} minutes", minutes).replace("{n}", minutes.toString())
    else -> ntr("Every {n} hour", "Every {n} hours", minutes / 60).replace("{n}", (minutes / 60).toString())
}

/** How account `a`'s sync goes, under its name; none for the local one. */
@Composable
private fun status(model: Main, a: Account): String? {
    val state by model.notav.syncState.collectAsStateWithLifecycle()
    val errors by model.notav.syncErrors.collectAsStateWithLifecycle()
    val last by model.notav.lastSync.collectAsStateWithLifecycle()
    val connected = load(model, a.id) { model.core.connected(a.id) }
    return when {
        a.kind == AccountKind.LOCAL -> null
        !a.enabled -> ptr("account", "Off")
        !connected -> tr("Not connected")
        state == SyncState.Syncing -> tr("Syncing…")
        errors[a.id] != null -> tr("Sync failed: {error}").replace("{error}", errors.getValue(a.id))
        last[a.id] != null -> lastSynced(last.getValue(a.id))
        else -> tr("Not synced yet")
    }
}

private fun lastSynced(seconds: Long) =
    tr("Last synced: {time}").replace("{time}", DateFormat.getTimeInstance(DateFormat.SHORT).format(Date(seconds * 1000)))

/**
 * An account's page, as the Linux app's: on or off, its name, the connection (signing in again),
 * its notes folder, the server's status and Remove.
 */
@Composable
private fun AccountPage(model: Main, id: Long, back: () -> Unit) {
    val core = model.core
    val account = load(model, id) { core.account(id) }
    if (account == null) {
        // Removed: back to the accounts.
        LaunchedEffect(Unit) { model.page.value = Page.Accounts }
        return
    }
    var removing by remember { mutableStateOf<String?>(null) }
    Frame(accountName(account), back) {
        Section(null) {
            Item(
                headlineContent = { Text(tr("Use This Account")) },
                supportingContent = { Text(tr("Off hides its notes and stops syncing them; nothing is deleted")) },
                trailingContent = { Switch(account.enabled, { model.setEnabled(id, it) }) },
                modifier = Modifier.clickable { model.setEnabled(id, !account.enabled) },
            )
            ApplyField(account.name, tr("Name"), Icons.Outlined.Badge) { model.renameAccount(id, it) }
        }
        if (account.kind == AccountKind.NEXTCLOUD) {
            val connected = load(model, id) { core.connected(id) }
            Section(tr("Connection")) {
                Item(
                    headlineContent = {
                        Text(if (connected) tr("Signed in as {name}").replace("{name}", account.user) else tr("Not connected"))
                    },
                    supportingContent = { Text(account.url) },
                )
                Item(
                    headlineContent = { Text(tr("Sign In Again")) },
                    leadingContent = { Icon(Icons.AutoMirrored.Outlined.Login, null) },
                    modifier = Modifier.clickable { model.page.value = Page.Reconnect(id) },
                )
            }
            NotesFolderSection(model, id)
            if (connected) ServerStatus(model, account)
        }
        Section(null) {
            Item(
                headlineContent = { Text(tr("Remove Account"), color = MaterialTheme.colorScheme.error) },
                leadingContent = { Icon(Icons.Outlined.Delete, null, tint = MaterialTheme.colorScheme.error) },
                modifier = Modifier.clickable { removing = "" },
            )
        }
    }
    // Asked first; when what waits for the server cannot go up, asked again (`removing` the why).
    removing?.let { why ->
        val first = why.isEmpty()
        AlertDialog(
            onDismissRequest = { removing = null },
            title = {
                Text(
                    if (first) tr("Remove {name}?").replace("{name}", accountName(account))
                    else tr("Remove Without Uploading?"),
                )
            },
            text = {
                Text(
                    when {
                        !first -> tr("{error}. Changes made here that the server does not have yet will be lost.")
                            .replace("{error}", why)
                        account.kind == AccountKind.LOCAL -> tr(
                            "Its notes are deleted from this device, with everything in them. This cannot be " +
                                "undone.",
                        )
                        else -> tr(
                            "Its notes leave this device; they stay on the server. Switch it off instead to " +
                                "only hide them.",
                        )
                    },
                )
            },
            confirmButton = {
                TextButton(onClick = {
                    removing = null
                    model.removeAccount(id, !first) { removing = it }
                }) { Text(if (first) tr("Remove") else tr("Remove Anyway"), color = MaterialTheme.colorScheme.error) }
            },
            dismissButton = { TextButton(onClick = { removing = null }) { Text(tr("Cancel")) } },
        )
    }
}

/** Account `id`'s notes folder: another one asks whether the notes move along, when there are some. */
@Composable
private fun NotesFolderSection(model: Main, id: Long) {
    val core = model.core
    val folder = load(model, id) { core.notesFolder(id) }
    var asking by remember { mutableStateOf<String?>(null) }
    // Cancelled: the folder there is, again.
    var resets by remember { mutableStateOf(0) }
    val cancel: () -> Unit = {
        asking = null
        resets++
    }
    Section(tr("Notes")) {
        ApplyField(folder, tr("Notes folder in Nextcloud Files"), Icons.Outlined.Folder, resets) { typed ->
            val f = typed.trim().trim('/')
            val chosen = core.chosenNotesFolder(id) != null
            when {
                f.isEmpty() || f == folder && chosen -> {}
                chosen && core.notes().any { it.account == id } -> asking = f
                else -> model.setNotesFolder(id, f, false)
            }
        }
    }
    asking?.let { f ->
        AlertDialog(
            onDismissRequest = cancel,
            title = { Text(tr("Move the Notes?")) },
            text = {
                Text(
                    tr("Move your notes from “{old}” to “{new}”, or leave them there and show the notes in “{new}”?")
                        .replace("{old}", folder).replace("{new}", f),
                )
            },
            confirmButton = {
                TextButton(onClick = {
                    asking = null
                    model.setNotesFolder(id, f, true)
                }) { Text(tr("Move")) }
            },
            dismissButton = {
                Row {
                    TextButton(onClick = cancel) { Text(tr("Cancel")) }
                    TextButton(onClick = {
                        asking = null
                        model.setNotesFolder(id, f, false)
                    }) { Text(tr("Leave Them")) }
                }
            },
        )
    }
}

/** How the server (asked when the page opens) and the account's sync are doing. */
@Composable
private fun ServerStatus(model: Main, account: Account) {
    val id = account.id
    // (icon, text): the icon none while it is being asked.
    val server by produceState<Pair<ImageVector?, String>>(null to tr("Checking…"), id) {
        value = withContext(Dispatchers.IO) {
            try {
                val info = model.core.serverInfo(id)
                val name = "${info.product} ${info.version}"
                if (info.maintenance) Icons.Outlined.WarningAmber to tr("{server}, in maintenance mode").replace("{server}", name)
                else Icons.Outlined.CheckCircle to name
            } catch (e: Exception) {
                Icons.Outlined.ErrorOutline to tr("Cannot reach the server: {error}").replace("{error}", reason(e))
            }
        }
    }
    val state by model.notav.syncState.collectAsStateWithLifecycle()
    val errors by model.notav.syncErrors.collectAsStateWithLifecycle()
    val last by model.notav.lastSync.collectAsStateWithLifecycle()
    val chosen = load(model, id) { model.core.chosenNotesFolder(id) != null }
    val sync: Pair<ImageVector?, String> = when {
        state == SyncState.Syncing -> null to tr("Syncing…")
        !chosen -> Icons.Outlined.WarningAmber to tr("Choose its notes folder below to sync it")
        errors[id] != null -> Icons.Outlined.ErrorOutline to tr("Failed: {error}").replace("{error}", errors.getValue(id))
        last[id] != null -> Icons.Outlined.CheckCircle to lastSynced(last.getValue(id))
        else -> null to tr("Not synced yet")
    }
    Section(tr("Server Status")) {
        listOf(tr("Server") to server, tr("Sync") to sync).forEach { (title, s) ->
            val (icon, text) = s
            Item(
                headlineContent = { Text(title) },
                supportingContent = { Text(text) },
                leadingContent = icon?.let {
                    {
                        Icon(
                            it,
                            null,
                            tint = when (it) {
                                Icons.Outlined.CheckCircle -> MaterialTheme.colorScheme.primary
                                Icons.Outlined.ErrorOutline -> MaterialTheme.colorScheme.error
                                else -> MaterialTheme.colorScheme.tertiary
                            },
                        )
                    }
                },
            )
        }
    }
}

/** Notav's version, source, issues and license, and the Support links, as the Linux app's About. */
@Composable
private fun About() {
    val context = LocalContext.current
    val version = remember { context.packageManager.getPackageInfo(context.packageName, 0).versionName ?: "" }
    val open = { url: String -> context.startActivity(Intent(Intent.ACTION_VIEW, url.toUri())) }
    Section(null) {
        Item(headlineContent = { Text("Notav") }, supportingContent = { Text(tr("Version {version}").replace("{version}", version)) })
        Item(
            headlineContent = { Text(tr("Source Code")) },
            supportingContent = { Text("github.com/AperteDev/notav") },
            modifier = Modifier.clickable { open("https://github.com/AperteDev/notav") },
        )
        Item(
            headlineContent = { Text(tr("Report an Issue")) },
            modifier = Modifier.clickable { open("https://github.com/AperteDev/notav/issues") },
        )
        Item(
            headlineContent = { Text(tr("License")) },
            supportingContent = { Text("GNU General Public License 3.0 or later") },
            modifier = Modifier.clickable { open("https://www.gnu.org/licenses/gpl-3.0.html") },
        )
    }
    // As the Linux app's About → Support (`DONATIONS` in its window.rs).
    Section(tr("Support")) {
        listOf(
            Triple(tr("Support on Ko-fi"), "ko-fi.com/N5E229GJPZ", Icons.Outlined.LocalCafe),
            Triple(tr("Sponsor on GitHub"), "github.com/sponsors/SelfRef", Icons.Outlined.FavoriteBorder),
            Triple(tr("Donate on Liberapay"), "liberapay.com/SelfRef/donate", Icons.Outlined.VolunteerActivism),
        ).forEach { (title, url, icon) ->
            Item(
                headlineContent = { Text(title) },
                supportingContent = { Text(url) },
                leadingContent = { Icon(icon, null) },
                modifier = Modifier.clickable { open("https://$url") },
            )
        }
    }
    Note(tr("Notav is developed in my free time. If it keeps your notes in order, you can support its development."))
}
