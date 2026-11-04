package dev.aperte.notav

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import dev.aperte.notav.core.Account
import dev.aperte.notav.core.AccountKind
import dev.aperte.notav.core.LoginFlow
import dev.aperte.notav.core.Note
import dev.aperte.notav.core.NotavException
import dev.aperte.notav.core.loginFlowPoll
import dev.aperte.notav.core.loginFlowStart
import dev.aperte.notav.core.passwordLogin
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.flowOn
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch

/** Where signing in is. */
sealed interface SignIn {
    data object Idle : SignIn
    data object Working : SignIn
    /** The browser is to open `url` (Login Flow v2); polling meanwhile. */
    data class Browser(val url: String) : SignIn
    data class Failed(val message: String) : SignIn
}

/** What the app shows, read again on every change. */
data class Screen(
    val accounts: List<Account>,
    val notes: List<Note>,
    /** A Nextcloud account added here whose notes folder is not chosen yet: its step shows. */
    val needsFolder: Account?,
)

class Main(app: Application) : AndroidViewModel(app) {
    val notav = app as NotavApp
    val core get() = notav.core

    val screen: StateFlow<Screen?> = combine(notav.ready, notav.changes) { ready, _ -> ready }
        .map { ready ->
            if (!ready) return@map null
            val accounts = core.accounts()
            Screen(
                accounts,
                core.notes(),
                accounts.firstOrNull { it.kind == AccountKind.NEXTCLOUD && core.chosenNotesFolder(it.id) == null },
            )
        }
        .flowOn(Dispatchers.IO)
        .stateIn(viewModelScope, SharingStarted.Eagerly, null)

    val signIn = MutableStateFlow<SignIn>(SignIn.Idle)
    private var polling: Job? = null

    /** The account whose first sync the welcome page shows, while it runs. */
    val firstSync = MutableStateFlow<Long?>(null)

    /** The activity is in front: failing to reach the server then is an answer, behind the
     * browser it is not (Android cuts off apps in the background). */
    var inFront = false

    private fun act(block: suspend () -> Unit) = viewModelScope.launch(Dispatchers.IO) { block() }

    /** Notes on this device only: the local account, with a note to start from. */
    fun useLocally() = act {
        if (core.accounts().none { it.kind == AccountKind.LOCAL }) {
            val id = core.addAccount(AccountKind.LOCAL, "", "", "")
            runCatching {
                core.addNote(
                    id,
                    "",
                    tr("Welcome to Notav"),
                    tr(
                        "Your notes stay on this device. They are written in Markdown: # starts a " +
                            "heading, - a list, - [ ] a task, and [[Title]] links another note.",
                    ),
                )
            }
        }
        notav.changed()
    }

    /** Sign in with the browser (Login Flow v2): `SignIn.Browser` asks the UI to open it. */
    fun signInWithBrowser(server: String) {
        if (server.isBlank()) {
            signIn.value = SignIn.Failed(tr("Enter the Nextcloud server first"))
            return
        }
        polling?.cancel()
        signIn.value = SignIn.Working
        polling = viewModelScope.launch(Dispatchers.IO) {
            val flow: LoginFlow = try {
                loginFlowStart(server.trim())
            } catch (e: Exception) {
                signIn.value = SignIn.Failed(tr("Cannot start login: {error}").replace("{error}", reason(e)))
                return@launch
            }
            signIn.value = SignIn.Browser(flow.loginUrl)
            // Nextcloud keeps a login flow for 20 minutes.
            var failures = 0
            repeat(20 * 60 / 2) {
                delay(2000)
                val credentials = try {
                    loginFlowPoll(flow).also { failures = 0 }
                } catch (e: Exception) {
                    if (inFront && ++failures >= 3) {
                        signIn.value = SignIn.Failed(tr("Login failed: {error}").replace("{error}", reason(e)))
                        return@launch
                    }
                    null
                }
                if (credentials != null) {
                    connect(credentials.server, credentials.loginName, credentials.appPassword)
                    return@launch
                }
            }
            signIn.value = SignIn.Failed(tr("Login timed out"))
        }
    }

    /** Sign in with a login name and a password or app password. */
    fun signInWithPassword(server: String, login: String, password: String) {
        if (server.isBlank()) {
            signIn.value = SignIn.Failed(tr("Enter the Nextcloud server first"))
            return
        }
        polling?.cancel()
        signIn.value = SignIn.Working
        polling = viewModelScope.launch(Dispatchers.IO) {
            try {
                val c = passwordLogin(server.trim(), login.trim(), password)
                connect(c.server, c.loginName, c.appPassword)
            } catch (e: Exception) {
                signIn.value = SignIn.Failed(tr("Sign-in failed: {error}").replace("{error}", reason(e)))
            }
        }
    }

    fun cancelSignIn() {
        polling?.cancel()
        signIn.value = SignIn.Idle
    }

    /** Add the account (the first one: settings sync through it); its notes folder is next. */
    private fun connect(server: String, user: String, password: String) {
        try {
            val first = core.accounts().none { it.kind == AccountKind.NEXTCLOUD }
            core.addNextcloud(notav.passwords, server, user, password, first && core.settingsAccount() == null)
            signIn.value = SignIn.Idle
            notav.changed()
        } catch (e: Exception) {
            signIn.value = SignIn.Failed(reason(e))
        }
    }

    /** The notes folder to offer for account `id`: the one other devices use, else Nextcloud
     * Notes' own. Blocking. */
    fun suggestedFolder(id: Long): String = core.suggestedNotesFolder(id)

    /**
     * Use `folder` for account `id`'s notes; on the welcome page (no other notes yet) its first
     * sync follows, its progress shown. A failed one is left to the sync worker, which tries
     * again and tells why. `failed` gets why the folder was not taken.
     */
    fun chooseFolder(id: Long, folder: String, failed: (String) -> Unit) = act {
        val welcome = core.accounts().none { it.id != id && (it.kind == AccountKind.LOCAL || core.chosenNotesFolder(it.id) != null) }
        try {
            core.setNotesFolder(id, folder.trim(), false)
        } catch (e: NotavException.Invalid) {
            failed(tr("Not a folder name"))
            return@act
        } catch (e: Exception) {
            failed(reason(e))
            return@act
        }
        if (welcome) {
            firstSync.value = id
            notav.changed()
            runCatching { core.syncAccount(id) }
            firstSync.value = null
        }
        notav.changed()
        notav.syncNow()
    }

    /** Back from the notes folder step: the account just added goes again, with its password. */
    fun dropAdded(id: Long) = act {
        runCatching { core.removeAccount(notav.passwords, id, true) }
        notav.changed()
    }
}
