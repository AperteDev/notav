package dev.aperte.notav

import android.content.Context
import androidx.work.Constraints
import androidx.work.CoroutineWorker
import androidx.work.ExistingWorkPolicy
import androidx.work.NetworkType
import androidx.work.OneTimeWorkRequestBuilder
import androidx.work.WorkManager
import androidx.work.WorkerParameters

/** The optional sync in the background (`Upload.background`): what is here and there in step. */
class Background(context: Context, params: WorkerParameters) : CoroutineWorker(context, params) {
    override suspend fun doWork(): Result {
        val app = applicationContext as NotavApp
        app.awaitReady()
        app.syncAll()
        return Result.success()
    }
}

/**
 * The app was left with changes waiting for the server: one sync to send them, once there is a
 * network, as the Linux app syncs before it quits. Nothing else runs in the background unless
 * the background sync is switched on (`background`).
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
        /** How often the background sync may run, in minutes; 0 is off, the default. */
        val INTERVALS = listOf(0L, 15L, 30L, 60L, 360L, 1440L)

        /**
         * The device-only `background_sync`, minutes between full syncs while Notav is closed,
         * once there is a network (Android may run them later); 0 (or none), no sync.
         */
        fun background(app: NotavApp, minutes: Long) {
            val work = WorkManager.getInstance(app)
            if (minutes <= 0) {
                work.cancelUniqueWork("background")
                return
            }
            val request = androidx.work.PeriodicWorkRequestBuilder<Background>(minutes, java.util.concurrent.TimeUnit.MINUTES)
                .setConstraints(Constraints.Builder().setRequiredNetworkType(NetworkType.CONNECTED).build())
                .build()
            work.enqueueUniquePeriodicWork("background", androidx.work.ExistingPeriodicWorkPolicy.UPDATE, request)
        }

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
