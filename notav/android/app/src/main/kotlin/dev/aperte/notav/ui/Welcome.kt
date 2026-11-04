package dev.aperte.notav.ui

import androidx.activity.compose.BackHandler
import androidx.browser.customtabs.CustomTabsIntent
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowRight
import androidx.compose.material.icons.outlined.Cloud
import androidx.compose.material.icons.outlined.Folder
import androidx.compose.material.icons.outlined.Key
import androidx.compose.material.icons.outlined.Password
import androidx.compose.material.icons.outlined.Person
import androidx.compose.material.icons.outlined.PhoneAndroid
import androidx.compose.material.icons.outlined.Visibility
import androidx.compose.material.icons.outlined.VisibilityOff
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearWavyProgressIndicator
import androidx.compose.material3.LoadingIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.unit.dp
import androidx.core.net.toUri
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import dev.aperte.notav.Main
import dev.aperte.notav.SignIn
import dev.aperte.notav.accountName
import dev.aperte.notav.core.Account
import dev.aperte.notav.core.AccountKind
import dev.aperte.notav.ntr
import dev.aperte.notav.tr
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/** A step of adding an account: a page of its own, its content no wider than [CONTENT_WIDTH]. */
@Composable
fun StepPage(content: @Composable ColumnScope.() -> Unit) {
    Surface(Modifier.fillMaxSize()) {
        Box(Modifier.fillMaxSize(), Alignment.TopCenter) {
            Column(
                Modifier.widthIn(max = CONTENT_WIDTH).fillMaxWidth()
                    .statusBarsPadding().navigationBarsPadding().imePadding()
                    .verticalScroll(rememberScrollState()).padding(24.dp),
                verticalArrangement = Arrangement.spacedBy(16.dp),
            ) {
                Spacer(Modifier.height(24.dp))
                content()
            }
        }
    }
}

@Composable
private fun Title(text: String) = Text(text, style = MaterialTheme.typography.headlineMedium)

/** No account yet: what Notav does, then the account to add, as the Linux app's welcome page. */
@Composable
fun Welcome(model: Main) = StepPage {
    Title(tr("Welcome to Notav"))
    AccountChoice(model, welcome = true)
}

/**
 * The kind of account to add, then that account: a server to sign in to, or notes on this device
 * only. On the welcome page and in Preferences → Accounts.
 */
@Composable
fun AccountChoice(model: Main, welcome: Boolean) {
    var server by rememberSaveable { mutableStateOf<String?>(null) }
    val entered = server
    if (entered == null) {
        Text(tr("Notes and a journal kept as Markdown files in your Nextcloud, or on this device"))
        Group {
            Choice(tr("Nextcloud Server"), tr("Sign in with the browser or an app password"), Icons.Outlined.Cloud) {
                server = ""
            }
            if (model.screen.value?.accounts.orEmpty().none { it.kind == AccountKind.LOCAL }) {
                Choice(tr("On This Device Only"), tr("Notes that stay on this device"), Icons.Outlined.PhoneAndroid) {
                    model.useLocally()
                }
            }
        }
        if (welcome) {
            Text(
                tr("You can add more accounts later in Preferences"),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    } else {
        BackHandler {
            model.cancelSignIn()
            server = null
        }
        SignInForm(model, entered) { server = it }
    }
}

@Composable
private fun Choice(title: String, description: String, icon: ImageVector, onClick: () -> Unit) = Item(
    headlineContent = { Text(title) },
    supportingContent = { Text(description) },
    leadingContent = { Icon(icon, null) },
    trailingContent = { Icon(Icons.AutoMirrored.Filled.KeyboardArrowRight, null) },
    modifier = Modifier.clickable(onClick = onClick),
)

/** A server to sign in to, with the browser or an app password. Todav's SignInForm. */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun SignInForm(model: Main, server: String, onServer: (String) -> Unit) {
    val state by model.signIn.collectAsStateWithLifecycle()
    var password by remember { mutableStateOf(false) }
    val context = LocalContext.current
    val s = state
    if (s is SignIn.Browser) {
        LaunchedEffect(s.url) { CustomTabsIntent.Builder().build().launchUrl(context, s.url.toUri()) }
    }
    Text(tr("Sign in with the browser or an app password"))
    Group {
        FieldItem(
            server,
            onServer,
            tr("Nextcloud server"),
            icon = Icons.Outlined.Cloud,
            singleLine = true,
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri, imeAction = ImeAction.Go),
            keyboardActions = KeyboardActions(onGo = { model.signInWithBrowser(server) }),
        )
    }
    when (s) {
        SignIn.Working, is SignIn.Browser -> Row(
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            LoadingIndicator(Modifier.size(40.dp))
            Text(tr("Waiting for Sign-In…"), Modifier.weight(1f))
            TextButton(onClick = model::cancelSignIn) { Text(tr("Cancel")) }
        }
        else -> {
            if (s is SignIn.Failed) Text(s.message, color = MaterialTheme.colorScheme.error)
            Button(onClick = { model.signInWithBrowser(server) }, Modifier.fillMaxWidth()) {
                Text(tr("Sign In with Browser"))
            }
            OutlinedButton(onClick = { password = true }, Modifier.fillMaxWidth()) {
                Text(tr("Sign In with Password…"))
            }
        }
    }
    if (password) PasswordDialog(onDismiss = { password = false }) { name, secret ->
        password = false
        model.signInWithPassword(server, name, secret)
    }
}

@Composable
private fun PasswordDialog(onDismiss: () -> Unit, onConnect: (String, String) -> Unit) {
    var login by rememberSaveable { mutableStateOf("") }
    var secret by remember { mutableStateOf("") }
    var shown by remember { mutableStateOf(false) }
    val ready = login.isNotBlank() && secret.isNotEmpty()
    val field = MaterialTheme.colorScheme.surfaceContainerHighest
    AlertDialog(
        onDismissRequest = onDismiss,
        icon = { Icon(Icons.Outlined.Key, null) },
        title = { Text(tr("Sign In with Password")) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(16.dp)) {
                Text(tr("Create an app password in Nextcloud under Settings → Security"))
                Group {
                    FieldItem(
                        login,
                        { login = it },
                        tr("Login name"),
                        icon = Icons.Outlined.Person,
                        singleLine = true,
                        keyboardOptions = KeyboardOptions(imeAction = ImeAction.Next),
                        containerColor = field,
                    )
                    FieldItem(
                        secret,
                        { secret = it },
                        tr("App password"),
                        icon = Icons.Outlined.Password,
                        trailing = {
                            val label = if (shown) tr("Hide Password") else tr("Show Password")
                            Tip(label) {
                                IconButton(onClick = { shown = !shown }) {
                                    Icon(if (shown) Icons.Outlined.VisibilityOff else Icons.Outlined.Visibility, label)
                                }
                            }
                        },
                        singleLine = true,
                        visualTransformation = if (shown) VisualTransformation.None else PasswordVisualTransformation(),
                        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password, imeAction = ImeAction.Done),
                        keyboardActions = KeyboardActions(onDone = { if (ready) onConnect(login, secret) }),
                        containerColor = field,
                    )
                }
            }
        },
        confirmButton = {
            Button(onClick = { onConnect(login, secret) }, enabled = ready) { Text(tr("Connect")) }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text(tr("Cancel")) } },
    )
}

/**
 * The last step for `account`, just signed in: its notes folder, suggested by the server (the
 * one other devices use, else Nextcloud Notes' own). Back drops the account.
 */
@Composable
fun NotesFolder(model: Main, account: Account) = StepPage {
    BackHandler { model.dropAdded(account.id) }
    var folder by rememberSaveable(account.id) { mutableStateOf<String?>(null) }
    var error by remember { mutableStateOf<String?>(null) }
    var working by remember { mutableStateOf(false) }
    LaunchedEffect(account.id) {
        if (folder == null) folder = withContext(Dispatchers.IO) { model.suggestedFolder(account.id) }
    }
    val choose = {
        folder?.takeIf { it.isNotBlank() && !working }?.let {
            working = true
            error = null
            model.chooseFolder(account.id, it) { why ->
                error = why
                working = false
            }
        }
    }
    Title(tr("Notes Folder"))
    Text(accountName(account), color = MaterialTheme.colorScheme.onSurfaceVariant)
    Text(
        tr(
            "The folder in Nextcloud Files your notes are kept in. The Nextcloud Notes app uses the " +
                "same one; it is created if it does not exist.",
        ),
    )
    Group {
        FieldItem(
            folder.orEmpty(),
            { folder = it },
            tr("Folder"),
            icon = Icons.Outlined.Folder,
            singleLine = true,
            enabled = folder != null && !working,
            keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
            keyboardActions = KeyboardActions(onDone = { choose() }),
        )
    }
    error?.let { Text(it, color = MaterialTheme.colorScheme.error) }
    Button(onClick = { choose() }, Modifier.fillMaxWidth(), enabled = folder != null && !working) {
        Text(tr("Continue"))
    }
}

/** The welcome page's first sync: how far it got, once it counts the notes. */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun FirstSync(model: Main, account: Long) = StepPage {
    val progress by model.notav.progress.collectAsStateWithLifecycle()
    val p = progress?.takeIf { it.account == account && it.total > 0 }
    Title(tr("Downloading Notes"))
    Text(
        tr(
            "Your notes are copied to this device, to work on them offline too. Later syncs bring " +
                "only what changed.",
        ),
    )
    if (p == null) {
        LinearWavyProgressIndicator(Modifier.fillMaxWidth())
        Text(tr("Looking for notes…"))
    } else {
        LinearWavyProgressIndicator(progress = { p.done.toFloat() / p.total }, modifier = Modifier.fillMaxWidth())
        Text(
            ntr("{done} of {total} note", "{done} of {total} notes", p.total.toLong())
                .replace("{done}", p.done.toString())
                .replace("{total}", p.total.toString()),
        )
    }
}
