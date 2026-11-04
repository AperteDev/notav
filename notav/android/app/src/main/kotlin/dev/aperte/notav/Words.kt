package dev.aperte.notav

import dev.aperte.notav.core.Account
import dev.aperte.notav.core.AccountKind
import dev.aperte.notav.core.NotavException
import dev.aperte.notav.core.accountHost

/** What an account is called: the name given to it, else "user@host", or "On This Device". */
fun accountName(a: Account): String = when {
    a.name.isNotEmpty() -> a.name
    a.kind == AccountKind.LOCAL -> tr("On This Device")
    else -> "${a.user}@${accountHost(a)}"
}

/** Why a core call failed, as the user reads it (the Linux app's words). */
fun reason(e: Throwable): String = when (e) {
    is NotavException.Auth -> tr("Wrong login name or password")
    is NotavException.Exists -> if (e.reason == "account") tr("This account is already added") else e.reason
    is NotavException.NotFound -> e.reason
    is NotavException.Http -> "HTTP ${e.code}: ${e.reason}"
    is NotavException.Net -> e.reason
    is NotavException.Parse -> e.reason
    is NotavException.Db -> e.reason
    is NotavException.Newer ->
        tr("It was saved by a newer version of Notav. Update Notav to open it; this version leaves it as it is.")
    is NotavException.Gone -> tr("Connecting failed")
    is NotavException.SavePassword -> tr("Cannot save password: {error}").replace("{error}", e.reason)
    is NotavException.Upload ->
        tr("Cannot upload pending changes first: {error}").replace("{error}", e.reason)
    is NotavException.NotUploaded ->
        tr("It has changes not uploaded yet: sign in again to upload them")
    else -> e.message ?: e.toString()
}
