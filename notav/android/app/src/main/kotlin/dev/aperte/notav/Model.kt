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
import kotlinx.coroutines.flow.first
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
    /** The notes of the accounts switched on. */
    val notes: List<Note>,
    /** A Nextcloud account added here whose notes folder is not chosen yet: its step shows. */
    val needsFolder: Account?,
    /** Each account switched on with its folders. */
    val folders: Map<Long, List<String>>,
    val rows: List<Row>,
)

/** A page over the notes: Settings, its pages (as the Linux app's Preferences), an account's. */
sealed interface Page {
    data object Settings : Page
    data object General : Page
    data object Editor : Page
    data object Journal : Page
    data object Accounts : Page
    data object About : Page
    data class Account(val id: Long) : Page
    data object AddAccount : Page
    /** Signing in again to account `id`. */
    data class Reconnect(val id: Long) : Page

    /** The page Back goes to; none, the notes. */
    val parent: Page?
        get() = when (this) {
            Settings -> null
            General, Editor, Journal, Accounts, About -> Settings
            is Account, AddAccount -> Accounts
            is Reconnect -> Account(id)
        }
}

/** A snackbar's text, with the change its Undo reverts, or the lock its Unlock… removes. */
data class Message(val text: String, val undo: ULong? = null, val unlock: dev.aperte.notav.core.Locked? = null)

/** A note's title from its path: the file name without its extension. */
fun titleOf(path: String): String = path.substringAfterLast('/').let { f -> f.substringBeforeLast('.', f) }

class Main(app: Application) : AndroidViewModel(app) {
    val notav = app as NotavApp
    val core get() = notav.core

    /** The open folders and closed accounts (`folderKey`), kept on this device as on Linux. */
    val expanded = MutableStateFlow<Set<String>>(emptySet())

    val screen: StateFlow<Screen?> = combine(notav.ready, notav.changes, expanded) { ready, _, open -> ready to open }
        .map { (ready, open) ->
            if (!ready) return@map null
            val accounts = core.accounts()
            val shown = accounts.filter { it.enabled }
            val folders = shown.associate { it.id to core.folders(it.id) }
            val notes = core.notes()
            Screen(
                accounts,
                notes,
                accounts.firstOrNull { it.kind == AccountKind.NEXTCLOUD && core.chosenNotesFolder(it.id) == null },
                folders,
                tree(shown.map { it.id to folders.getValue(it.id) }, notes, open),
            )
        }
        .flowOn(Dispatchers.IO)
        .stateIn(viewModelScope, SharingStarted.Eagerly, null)

    /** The note shown. */
    val open = MutableStateFlow<Long?>(null)

    /** A note was opened (the same one again too): the panes slide to it. */
    val showNote = kotlinx.coroutines.flow.MutableSharedFlow<Unit>(extraBufferCapacity = 1)

    /** Where the open note's text should go to (a heading picked in the outline), in UTF-16 units. */
    val jump = kotlinx.coroutines.flow.MutableSharedFlow<Int>(extraBufferCapacity = 1)

    /** The open note's headings as typed, for the outline. */
    val headings = MutableStateFlow<List<dev.aperte.notav.core.NoteHeading>>(emptyList())

    /** Where a new note goes: the account and folder last opened in the tree, or the open note's. */
    var target: Pair<Long, String>? = null

    val messages = kotlinx.coroutines.flow.MutableSharedFlow<Message>(extraBufferCapacity = 8)

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
        page.value = null
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

    /** Add the account (the first one: settings sync through it); its notes folder is next. Or,
     * signing in again (`page` is `Page.Reconnect`), the account's new app password or server. */
    private fun connect(server: String, user: String, password: String) {
        (page.value as? Page.Reconnect)?.let { again ->
            try {
                core.reconnect(notav.passwords, again.id, server, password)
                signIn.value = SignIn.Idle
                page.value = Page.Account(again.id)
                notav.changed()
                notav.syncNow()
            } catch (e: Exception) {
                signIn.value = SignIn.Failed(reason(e))
            }
            return
        }
        try {
            val first = core.accounts().none { it.kind == AccountKind.NEXTCLOUD }
            core.addNextcloud(notav.passwords, server, user, password, first && core.settingsAccount() == null)
            signIn.value = SignIn.Idle
            page.value = null
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

    // --- the tree ---------------------------------------------------------------------------

    /** A change was made: the screens read again, and it goes to the server. */
    private fun done(message: Message? = null) {
        notav.changed()
        notav.requestSync()
        message?.let { messages.tryEmit(it) }
    }

    private fun failed(e: Exception, exists: String = tr("A note with this name exists already")) {
        messages.tryEmit(
            Message(
                when (e) {
                    is NotavException.Exists -> exists
                    is NotavException.Invalid -> tr("Not a folder name")
                    else -> reason(e)
                },
            ),
        )
    }

    /** Open or close a folder (or an account's rows); new notes go there next. */
    fun toggle(item: Item) {
        val (account, folder) = item.place
        val key = folderKey(account, folder)
        expanded.value = if (key in expanded.value) expanded.value - key else expanded.value + key
        target = item.place
        act { core.setSetting("expanded_folders", expanded.value.sorted().joinToString("\n")) }
    }

    /** Open account `account` and its folders above `folder`, so a note in it shows. */
    private fun reveal(account: Long, folder: String) {
        val open = expanded.value.toMutableSet()
        open -= folderKey(account, "")
        var f = folder
        while (f.isNotEmpty()) {
            open += folderKey(account, f)
            f = f.substringBeforeLast('/', "")
        }
        if (open != expanded.value) {
            expanded.value = open
            act { core.setSetting("expanded_folders", open.sorted().joinToString("\n")) }
        }
    }

    fun openNote(note: Note) {
        open.value = note.id
        target = note.account to note.folder
        showNote.tryEmit(Unit)
    }

    /** Where something new goes without a place given: where the last one went, else the top of
     * the first account switched on. */
    fun newPlace(): Pair<Long, String>? = target?.takeIf { (a, _) -> screen.value?.folders?.containsKey(a) == true }
        ?: screen.value?.accounts?.firstOrNull { it.enabled }?.let { it.id to "" }

    /** A new, empty note in `place` (else `newPlace`), opened. */
    fun newNote(place: Pair<Long, String>? = null) = act {
        val (account, folder) = place ?: newPlace() ?: return@act
        try {
            val n = core.addNote(account, folder, tr("Untitled"), "")
            reveal(account, folder)
            openNote(n)
            done()
        } catch (e: Exception) {
            failed(e)
        }
    }

    /** A new folder `name` in `place`, shown open. */
    fun newFolder(place: Pair<Long, String>, name: String) = act {
        val (account, parent) = place
        val clean = name.trim().trim('/')
        val path = if (parent.isEmpty()) clean else "$parent/$clean"
        if (path in core.folders(account)) {
            messages.tryEmit(Message(tr("A folder with this name exists already")))
            return@act
        }
        try {
            core.addFolder(account, path)
            reveal(account, path)
            target = account to path
            done()
        } catch (e: Exception) {
            failed(e)
        }
    }

    fun renameFolder(folder: Item.Folder, name: String) = act {
        try {
            core.renameFolder(folder.account, folder.path, name.trim())
            done()
        } catch (e: Exception) {
            failed(e, tr("A folder with this name exists already"))
        }
    }

    fun renameNote(note: Note, title: String) = act {
        try {
            core.moveNote(note.id, note.folder, title.trim())
            done()
        } catch (e: Exception) {
            failed(e)
        }
    }

    /** Move a note or folder into `folder` of its account; the snackbar can undo it. */
    fun move(item: Item, folder: String) = act {
        try {
            when (item) {
                is Item.NoteRow -> {
                    core.moveNote(item.note.id, folder, item.note.title)
                    reveal(item.note.account, folder)
                }
                is Item.Folder -> {
                    core.moveFolder(item.account, item.path, folder)
                    reveal(item.account, folder)
                }
                is Item.Account -> return@act
            }
            done(Message(tr("Moved"), core.lastChange()))
        } catch (e: NotavException.Exists) {
            messages.tryEmit(
                Message(
                    if (item is Item.Folder) {
                        tr("A folder with this name exists in that folder already")
                    } else {
                        tr("A note with this name exists in that folder already")
                    },
                ),
            )
        } catch (e: Exception) {
            failed(e)
        }
    }

    /** Delete a note or a folder with everything in it; the snackbar can undo it. */
    fun delete(item: Item) = act {
        try {
            val name = when (item) {
                is Item.NoteRow -> item.note.title.also {
                    core.deleteNote(item.note.id)
                    if (open.value == item.note.id) open.value = null
                }
                is Item.Folder -> item.name.also { core.deleteFolder(item.account, item.path) }
                is Item.Account -> return@act
            }
            done(Message(tr("“{title}” deleted").replace("{title}", name), core.lastChange()))
        } catch (e: Exception) {
            failed(e)
        }
    }

    fun setFavorite(note: Note, on: Boolean) = act {
        try {
            core.setFavorite(note.id, on)
            done()
        } catch (e: Exception) {
            failed(e)
        }
    }

    // --- Preferences -------------------------------------------------------------------------

    val page = MutableStateFlow<Page?>(null)

    fun setSetting(key: String, value: String) = act {
        core.setSetting(key, value)
        notav.changed()
    }

    /** The optional background sync, every `minutes` or off (0; device-only, off by default). */
    fun setBackgroundSync(minutes: Long) = act {
        core.setSetting("background_sync", minutes.toString())
        Upload.background(notav, minutes)
        notav.changed()
    }

    fun renameAccount(id: Long, name: String) = act {
        core.renameAccount(id, name.trim())
        notav.changed()
    }

    /** Settings sync through account `id` (a Nextcloud one), or none; its settings come at once. */
    fun setSettingsAccount(id: Long?) = act {
        core.setSettingsAccount(id)
        notav.changed()
        notav.syncNow()
    }

    /** Account `id`'s notes in `folder` of its Files, `move`d there from the old one or not. */
    fun setNotesFolder(id: Long, folder: String, move: Boolean) = act {
        val text = try {
            core.setNotesFolder(id, folder, move)
            if (open.value?.let { core.note(it) } == null) open.value = null
            notav.changed()
            notav.syncNow()
            tr("Notes folder changed to “{folder}”").replace("{folder}", folder)
        } catch (e: NotavException.Invalid) {
            tr("Not a folder name")
        } catch (e: Exception) {
            tr("Notes folder not changed: {error}").replace("{error}", reason(e))
        }
        messages.tryEmit(Message(text))
    }

    /** The journal's folder, made in its account when there is none; empty, the default. */
    fun setJournalFolder(folder: String) = act {
        val f = folder.trim().trim('/')
        val account = core.journalAccount()
        if (f.isNotEmpty() && account != null) {
            try {
                core.addFolder(account, f)
            } catch (e: Exception) {
                failed(e)
                notav.changed() // the field shows the folder there is again
                return@act
            }
        }
        core.setSetting("journal_folder", f)
        notav.changed()
    }

    /** The note new daily notes copy, one in the journal's account; empty, none. */
    fun setJournalTemplate(name: String) = act {
        val t = name.trim().trim('/')
        val path = if (t.endsWith(".md")) t else "$t.md"
        val account = core.journalAccount()
        if (t.isNotEmpty() && core.notes().none { it.account == account && it.path == path }) {
            messages.tryEmit(Message(tr("No note “{name}”").replace("{name}", t)))
        } else {
            core.setSetting("journal_template", t)
        }
        notav.changed()
    }

    fun setEnabled(id: Long, on: Boolean) = act {
        core.setAccountEnabled(id, on)
        notav.changed()
        if (on) notav.syncNow()
    }

    /**
     * Remove account `id`; what waits for its server goes up first, unless `force`. `cannot`
     * gets why that failed, to ask whether to remove it anyway.
     */
    fun removeAccount(id: Long, force: Boolean, cannot: (String) -> Unit) = act {
        val name = core.account(id)?.let(::accountName) ?: return@act
        try {
            core.removeAccount(notav.passwords, id, force)
            if (open.value?.let { core.note(it) } == null) open.value = null
            target = null
            notav.changed()
            messages.tryEmit(Message(tr("{name} removed").replace("{name}", name)))
        } catch (e: NotavException.Upload) {
            cannot(reason(e))
        } catch (e: NotavException.NotUploaded) {
            cannot(reason(e))
        } catch (e: Exception) {
            failed(e)
        }
    }

    // --- the note ---------------------------------------------------------------------------

    /** Source mode instead of Editor mode: the device-only `source_mode`, as on Linux. */
    val sourceMode = MutableStateFlow(false)

    fun setSourceMode(on: Boolean) {
        sourceMode.value = on
        act { core.setSetting("source_mode", on.toString()) }
    }

    /** Save a note's text; it goes to the server with the next sync. On the app's scope, so a
     * note left (or the app closed) is saved all the same. */
    fun saveText(id: Long, text: String) = notav.scope.launch {
        try {
            core.setText(id, text)
            notav.requestSync()
        } catch (e: Exception) {
            messages.tryEmit(Message(tr("Cannot save the note: {error}").replace("{error}", reason(e))))
        }
    }

    /** The notes found for what is typed in the search (null: not searching). */
    val found = MutableStateFlow<List<Note>?>(null)
    private var searching: Job? = null

    /** Search titles and text for `query` (case and accents ignored); null stops searching. */
    fun search(query: String?) {
        searching?.cancel()
        if (query == null) {
            found.value = null
            return
        }
        searching = act {
            delay(200) // as typing goes on
            found.value = if (query.isBlank()) emptyList() else core.search(query)
        }
    }

    /** Today's note of the journal: the one there is, or a new one from the template. */
    fun today() = act {
        val d = java.time.LocalDate.now()
        try {
            val n = core.dailyNote(d.year, d.monthValue.toUInt(), d.dayOfMonth.toUInt())
            reveal(n.account, n.folder)
            openNote(n)
            done()
        } catch (e: Exception) {
            failed(e)
        }
    }

    /** Text shared from another app, as a new note where new notes go (`subject` its title, else
     * its first line). */
    fun shared(subject: String?, text: String) = act {
        screen.first { it != null } // shared as the app starts: once its accounts are read
        val (account, folder) = newPlace() ?: return@act
        val title = subject?.takeIf { it.isNotBlank() }
            ?: text.lineSequence().firstOrNull { it.isNotBlank() }?.trim()?.take(60)
            ?: tr("Untitled")
        try {
            val n = core.addNote(account, folder, title.replace('/', '-'), text)
            reveal(account, folder)
            openNote(n)
            done()
        } catch (e: Exception) {
            failed(e)
        }
    }

    /** Open the note a link in `from` names; made where the link puts it when there is none. */
    fun followLink(from: Note, target: String, wiki: Boolean) = act {
        try {
            val n = core.followLink(from.id, target, wiki)
            reveal(n.account, n.folder)
            openNote(n)
            done()
        } catch (e: Exception) {
            failed(e)
        }
    }

    /** Remove the lock on the server that a change to its note waits for; the sync then goes on. */
    fun unlock(lock: dev.aperte.notav.core.Locked) = act {
        val file = lock.file ?: return@act
        try {
            core.unlock(lock.account, file)
            notav.syncNow()
        } catch (e: Exception) {
            messages.tryEmit(
                Message(
                    tr("Cannot unlock “{title}”: {error}").replace("{title}", titleOf(lock.path)).replace("{error}", reason(e)),
                ),
            )
        }
    }

    fun undo(change: ULong?) = act {
        try {
            core.undo(change)
            done()
        } catch (e: Exception) {
            failed(e)
        }
    }

    init {
        act {
            notav.awaitReady()
            expanded.value = core.setting("expanded_folders").orEmpty().lines().filter { it.isNotEmpty() }.toSet()
            sourceMode.value = core.setting("source_mode") == "true"
        }
    }
}
