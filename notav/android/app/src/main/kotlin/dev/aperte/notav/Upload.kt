package dev.aperte.notav

import android.content.Context
import androidx.work.Constraints
import androidx.work.CoroutineWorker
import androidx.work.ExistingWorkPolicy
import androidx.work.NetworkType
import androidx.work.OneTimeWorkRequestBuilder
import androidx.work.WorkManager
import androidx.work.WorkerParameters

/**
 * The app was left with changes waiting for the server: one sync to send them, once there is a
 * network, as the Linux app syncs before it quits. Nothing else runs in the background.
 */
class Upload(context: Context, params: WorkerParameters) : CoroutineWorker(context, params) {
    override suspend fun doWork(): Result {
        val app = applicationContext as NotavApp
        app.awaitReady()
        if (!app.core.pending(null)) return Result.success()
        app.syncAll()
        // Still waiting (offline after all, or the server failed): try again later.
        return if (app.core.pending(null) && app.syncState.value is SyncState.Failed) Result.retry() else Result.success()
    }

    companion object {
        /** Unless nothing waits for the server. */
        fun schedule(app: NotavApp) {
            if (!app.core.pending(null)) return
            val request = OneTimeWorkRequestBuilder<Upload>()
                .setConstraints(Constraints.Builder().setRequiredNetworkType(NetworkType.CONNECTED).build())
                .build()
            WorkManager.getInstance(app).enqueueUniqueWork("upload", ExistingWorkPolicy.REPLACE, request)
        }
    }
}
