package dev.aperte.notav.ui

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.outlined.Login
import androidx.compose.material.icons.outlined.Add
import androidx.compose.material.icons.outlined.Cloud
import androidx.compose.material.icons.outlined.Delete
import androidx.compose.material.icons.outlined.MoreVert
import androidx.compose.material.icons.outlined.PhoneAndroid
import androidx.compose.material.icons.outlined.Check
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.ExposedDropdownMenu
import androidx.compose.material3.ExposedDropdownMenuAnchorType
import androidx.compose.material3.ExposedDropdownMenuBox
import androidx.compose.material3.ExposedDropdownMenuDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.MenuDefaults
import androidx.compose.material3.SelectableDropdownMenuItem
import dev.aperte.notav.Upload
import dev.aperte.notav.ntr
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import dev.aperte.notav.Main
import dev.aperte.notav.Page
import dev.aperte.notav.SyncState
import dev.aperte.notav.accountName
import dev.aperte.notav.core.Account
import dev.aperte.notav.core.AccountKind
import dev.aperte.notav.ptr
import dev.aperte.notav.tr
import java.text.DateFormat
import java.util.Date
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/** A page over the notes, with a back arrow: Preferences, adding an account, signing in again. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun PageView(model: Main, page: Page) {
    val back = {
        model.cancelSignIn()
        model.page.value = if (page == Page.Settings) null else Page.Settings
    }
    BackHandler(onBack = back)
    Scaffold(
        topBar = {
            TopAppBar(
                title = {
                    Text(
                        when (page) {
                            Page.Settings -> tr("Preferences")
                            Page.AddAccount -> tr("Add Account")
                            is Page.Reconnect -> tr("Sign In Again")
                        },
                    )
                },
                navigationIcon = { IconButton(onClick = back) { Icon(Icons.AutoMirrored.Filled.ArrowBack, null) } },
            )
        },
    ) { padding ->
        Box(Modifier.fillMaxSize().padding(padding), Alignment.TopCenter) {
            Column(
                Modifier.widthIn(max = CONTENT_WIDTH).fillMaxWidth().verticalScroll(rememberScrollState())
                    .navigationBarsPadding().padding(16.dp),
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                when (page) {
                    Page.Settings -> Preferences(model)
                    Page.AddAccount -> AccountChoice(model, welcome = false)
                    is Page.Reconnect -> {
                        val account = model.screen.value?.accounts?.firstOrNull { it.id == page.id }
                        var server by rememberSaveable { mutableStateOf(account?.url.orEmpty()) }
                        account?.let { Text(accountName(it), color = MaterialTheme.colorScheme.onSurfaceVariant) }
                        SignInForm(model, server) { server = it }
                    }
                }
            }
        }
    }
}

@Composable
private fun Title(text: String) = Text(
    text,
    style = MaterialTheme.typography.titleSmall,
    color = MaterialTheme.colorScheme.primary,
    modifier = Modifier.padding(start = 16.dp, top = 12.dp),
)

/** As the Linux app's Preferences: Appearance, Editor, Accounts, and the background sync. */
@Composable
private fun Preferences(model: Main) {
    val changes by model.notav.changes.collectAsStateWithLifecycle()
    val settings by produceState(emptyMap<String, String?>(), changes) {
        value = withContext(Dispatchers.IO) {
            listOf("color_scheme", "format_tables", "background_sync").associateWith { model.core.setting(it) }
        }
    }
    Title(tr("Appearance"))
    Group {
        Item(
            headlineContent = { Text(tr("Style")) },
            supportingContent = {
                val choices = listOf(
                    "system" to ptr("style", "System"),
                    "light" to ptr("style", "Light"),
                    "dark" to ptr("style", "Dark"),
                )
                val current = choices.indexOfFirst { it.first == settings["color_scheme"] }.coerceAtLeast(0)
                Connected(choices.map { it.second }, current, Modifier.fillMaxWidth().padding(top = 8.dp)) {
                    model.setSetting("color_scheme", choices[it].first)
                }
            },
        )
    }
    Title(tr("Editor"))
    Group {
        val on = settings["format_tables"] != "false"
        Item(
            headlineContent = { Text(tr("Format tables")) },
            supportingContent = { Text(tr("Keep a table’s columns lined up as you type and move between its rows and cells")) },
            trailingContent = { Switch(on, { model.setSetting("format_tables", it.toString()) }) },
            modifier = Modifier.clickable { model.setSetting("format_tables", (!on).toString()) },
        )
    }
    Title(tr("Accounts"))
    val screen by model.screen.collectAsStateWithLifecycle()
    Group {
        screen?.accounts?.forEach { AccountRow(model, it) }
        Item(
            headlineContent = { Text(tr("Add Account")) },
            leadingContent = { Icon(Icons.Outlined.Add, null) },
            modifier = Modifier.clickable { model.page.value = Page.AddAccount },
        )
    }
    Title(tr("Sync"))
    Group {
        val minutes = settings["background_sync"]?.toLongOrNull()?.takeIf { it in Upload.INTERVALS } ?: 0L
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

/**
 * A row naming the one of `options` picked (`selected`) under `title`, with `subtitle`; a tap
 * opens them in a menu below it (Material's select, an exposed dropdown menu).
 */
@OptIn(ExperimentalMaterial3Api::class, ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun Select(title: String, subtitle: String, options: List<String>, selected: Int, onPick: (Int) -> Unit) {
    var open by remember { mutableStateOf(false) }
    ExposedDropdownMenuBox(open, { open = it }) {
        Item(
            headlineContent = { Text(title) },
            supportingContent = {
                Column {
                    Text(options[selected], color = MaterialTheme.colorScheme.primary)
                    Text(subtitle)
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

/** An account: its name and how its sync goes, on or off; its menu signs in again or removes it. */
@Composable
private fun AccountRow(model: Main, a: Account) {
    val state by model.notav.syncState.collectAsStateWithLifecycle()
    val errors by model.notav.syncErrors.collectAsStateWithLifecycle()
    val last by model.notav.lastSync.collectAsStateWithLifecycle()
    var menu by remember { mutableStateOf(false) }
    var removing by remember { mutableStateOf<String?>(null) }
    val scope = rememberCoroutineScope()
    val status = when {
        a.kind == AccountKind.LOCAL -> null
        !a.enabled -> ptr("account", "Off")
        !model.core.connected(a.id) -> tr("Not connected")
        state == SyncState.Syncing -> tr("Syncing…")
        errors[a.id] != null -> tr("Sync failed: {error}").replace("{error}", errors.getValue(a.id))
        last[a.id] != null -> tr("Last synced: {time}")
            .replace("{time}", DateFormat.getTimeInstance(DateFormat.SHORT).format(Date(last.getValue(a.id) * 1000)))
        else -> tr("Not synced yet")
    }
    Item(
        headlineContent = { Text(accountName(a)) },
        supportingContent = status?.let { { Text(it) } },
        leadingContent = { Icon(if (a.kind == AccountKind.LOCAL) Icons.Outlined.PhoneAndroid else Icons.Outlined.Cloud, null) },
        trailingContent = {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Switch(a.enabled, { model.setEnabled(a.id, it) })
                Box {
                    IconButton(onClick = { menu = true }) { Icon(Icons.Outlined.MoreVert, tr("Account")) }
                    Menu(
                        menu,
                        { menu = false },
                        listOf(
                            listOfNotNull(
                                Entry(tr("Sign In Again"), Icons.AutoMirrored.Outlined.Login) {
                                    model.page.value = Page.Reconnect(a.id)
                                }.takeIf { a.kind == AccountKind.NEXTCLOUD },
                            ),
                            listOf(Entry(tr("Remove Account"), Icons.Outlined.Delete, danger = true) { removing = "" }),
                        ),
                    )
                }
            }
        },
    )
    // Asked first; when what waits for the server cannot go up, asked again (`removing` the why).
    removing?.let { why ->
        val first = why.isEmpty()
        AlertDialog(
            onDismissRequest = { removing = null },
            title = {
                Text(
                    if (first) tr("Remove {name}?").replace("{name}", accountName(a))
                    else tr("Remove Without Uploading?"),
                )
            },
            text = {
                Text(
                    when {
                        !first -> tr("{error}. Changes made here that the server does not have yet will be lost.")
                            .replace("{error}", why)
                        a.kind == AccountKind.LOCAL -> tr(
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
                    scope.launch { model.removeAccount(a.id, !first) { removing = it } }
                }) { Text(if (first) tr("Remove") else tr("Remove Anyway"), color = MaterialTheme.colorScheme.error) }
            },
            dismissButton = { TextButton(onClick = { removing = null }) { Text(tr("Cancel")) } },
        )
    }
}
