package dev.aperte.notav

import android.app.Application
import android.util.Log
import dev.aperte.notav.core.AccountKind
import dev.aperte.notav.core.Listener
import dev.aperte.notav.core.Locked
import dev.aperte.notav.core.Notav
import dev.aperte.notav.core.NotavException
import dev.aperte.notav.core.setTrustedCertificates
import java.security.KeyStore
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

private const val TAG = "Notav"

/** What the sync worker is doing. */
sealed interface SyncState {
    data object Idle : SyncState
    data object Syncing : SyncState
    /** What failed, naming the accounts. */
    data class Failed(val message: String) : SyncState
}

/** A sync of `account` went through `done` of `total` notes. */
data class Progress(val account: Long, val done: Int, val total: Int)

/**
 * The process: the core (database, accounts), the sync worker while the app is open, and what the
 * UI follows of them. Core calls block, so they run on [Dispatchers.IO]; its callbacks come from
 * its own threads. As the Linux app: no service in the background, only a last upload when the
 * app is left (`Upload`).
 */
class NotavApp : Application() {
    lateinit var core: Notav
        private set
    lateinit var passwords: Passwords
        private set

    /** Why the data could not be opened (left by a newer Notav, say): nothing else starts, and
     * the app shows only that. */
    var failed: Throwable? = null
        private set

    /** Bumped on every change to the data, here or synced: screens read again. */
    val changes = MutableStateFlow(0L)
    val syncState = MutableStateFlow<SyncState>(SyncState.Idle)
    val progress = MutableStateFlow<Progress?>(null)
    /** Conflict copies a sync made, by path. */
    val conflicts = MutableSharedFlow<List<String>>(extraBufferCapacity = 8)
    /** Changes that wait for a lock on the server. */
    val locked = MutableStateFlow<List<Locked>>(emptyList())
    /** Per account: when its last sync went through (unix time) and why its last one failed;
     * this run's only. */
    val lastSync = MutableStateFlow<Map<Long, Long>>(emptyMap())
    val syncErrors = MutableStateFlow<Map<Long, String>>(emptyMap())
    /** The accounts are connected: the first screen can show. */
    val ready = MutableStateFlow(false)

    val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private val requests = Channel<Unit>(Channel.CONFLATED)

    override fun onCreate() {
        super.onCreate()
        trustCertificates()
        I18n.load(this)
        passwords = Passwords(this)
        core = try {
            Notav.open(filesDir.resolve("notav").path)
        } catch (e: NotavException) {
            failed = e
            return
        }
        core.subscribe(object : Listener {
            override fun changed(paths: List<String>) = changed()
            override fun settingsChanged() = changed()
            override fun progress(account: Long, done: UInt, total: UInt) {
                progress.value = Progress(account, done.toInt(), total.toInt())
            }
        })
        scope.launch {
            connectAccounts()
            ready.value = true
            // Requests coalesce: one sync for a burst of them (typing saves every second or so).
            for (r in requests) {
                delay(1500)
                requests.tryReceive()
                if (core.accounts().none { core.syncing(it.id) }) continue
                syncAll()
            }
        }
    }

    fun changed() = changes.update { it + 1 }

    /** Fails at once when the data could not be opened. */
    suspend fun awaitReady() {
        failed?.let { throw IllegalStateException("data not opened", it) }
        ready.first { it }
    }

    /** A full sync soon; requests arriving while one runs are coalesced. */
    fun syncNow() {
        requests.trySend(Unit)
    }

    /** After an edit: sync if it left something for the server. */
    fun requestSync() = scope.launch {
        if (core.pending(null)) syncNow()
    }

    /**
     * Settings, then every account that syncs; what failed is told by account. Blocking; the
     * worker runs it, and `Upload` once the app is left.
     */
    @Synchronized
    fun syncAll() {
        syncState.value = SyncState.Syncing
        val failed = mutableListOf<String>()
        val errors = mutableMapOf<Long, String>()
        val conflicts = mutableListOf<String>()
        val locked = mutableListOf<Locked>()
        try {
            core.syncSettings()
        } catch (e: Exception) {
            failed += reason(e)
        }
        for (a in core.accounts()) {
            if (a.kind != AccountKind.NEXTCLOUD || !core.syncing(a.id)) continue
            try {
                val r = core.syncAccount(a.id)
                conflicts += r.conflicts
                locked += r.locked
                lastSync.update { it + (a.id to System.currentTimeMillis() / 1000) }
            } catch (e: Exception) {
                Log.w(TAG, "sync ${a.id}", e)
                failed += "${accountName(a)}: ${reason(e)}"
                errors[a.id] = reason(e)
            }
        }
        syncErrors.value = errors
        if (conflicts.isNotEmpty()) this.conflicts.tryEmit(conflicts)
        this.locked.value = locked
        progress.value = null
        syncState.value = if (failed.isEmpty()) SyncState.Idle else SyncState.Failed(failed.joinToString("; "))
    }

    /** Connect the Nextcloud accounts signed in here with their saved app passwords. */
    private fun connectAccounts() {
        for (a in core.accounts()) {
            if (a.kind != AccountKind.NEXTCLOUD) continue
            val password = passwords.load(a.url, a.user) ?: continue
            runCatching { core.connect(a.id, password) }
        }
    }

    /** HTTPS trusts what Android does, certificate authorities the user installed too (a
     * self-hosted server's own CA), not only the core's built-in ones. Read once per process:
     * one added later counts from the next start. */
    private fun trustCertificates() {
        runCatching {
            val store = KeyStore.getInstance("AndroidCAStore").apply { load(null) }
            setTrustedCertificates(store.aliases().toList().mapNotNull { store.getCertificate(it)?.encoded })
        }
    }
}
