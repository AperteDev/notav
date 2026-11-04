package dev.aperte.notav

import android.app.Application
import dev.aperte.notav.core.Listener
import dev.aperte.notav.core.Notav
import dev.aperte.notav.core.NotavException
import dev.aperte.notav.core.setTrustedCertificates
import java.security.KeyStore
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.update

/**
 * The process: the core (database, accounts) and what the UI follows of it. Core calls block, so
 * they run on [Dispatchers.IO]; its callbacks come from its own threads.
 */
class NotavApp : Application() {
    lateinit var core: Notav
        private set

    /** Why the data could not be opened (left by a newer Notav, say): nothing else starts, and
     * the app shows only that. */
    var failed: Throwable? = null
        private set

    /** Bumped on every change to the data, here or synced: screens read again. */
    val changes = MutableStateFlow(0L)

    val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)

    override fun onCreate() {
        super.onCreate()
        trustCertificates()
        I18n.load(this)
        core = try {
            Notav.open(filesDir.resolve("notav").path)
        } catch (e: NotavException) {
            failed = e
            return
        }
        core.subscribe(object : Listener {
            override fun changed(paths: List<String>) = changed()
            override fun settingsChanged() = changed()
            override fun progress(account: Long, done: UInt, total: UInt) {}
        })
    }

    fun changed() = changes.update { it + 1 }

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
