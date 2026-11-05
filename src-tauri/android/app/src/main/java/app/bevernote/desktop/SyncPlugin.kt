package app.bevernote.desktop

import android.app.Activity
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import android.os.Handler
import android.os.Looper
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.Plugin

@InvokeArg
class SyncServiceArgs {
  var message: String = "Synchronizing…"
  var current: Long = 0
  var total: Long = 0
}

/** Native-only bridge. It is intentionally not exposed as a frontend plugin. */
@TauriPlugin
class SyncPlugin(private val activity: Activity) : Plugin(activity) {
  companion object {
    private val lock = Object()
    private val handler = Handler(Looper.getMainLooper())
    private var pendingStart: Invoke? = null
    private var timeout: Runnable? = null

    fun acknowledgeStart() {
      val invoke = synchronized(lock) {
        timeout?.let(handler::removeCallbacks)
        timeout = null
        pendingStart.also { pendingStart = null }
      }
      invoke?.resolve()
    }

    fun failStart(message: String) {
      val invoke = synchronized(lock) {
        timeout?.let(handler::removeCallbacks)
        timeout = null
        pendingStart.also { pendingStart = null }
      }
      invoke?.reject(message)
    }
  }

  @Command
  fun start(invoke: Invoke) {
    val args = invoke.parseArgs(SyncServiceArgs::class.java)
    synchronized(lock) {
      if (pendingStart != null) {
        invoke.reject("A sync service is already starting")
        return
      }
      pendingStart = invoke
    }
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU &&
      activity.checkSelfPermission(android.Manifest.permission.POST_NOTIFICATIONS) !=
      PackageManager.PERMISSION_GRANTED
    ) {
      // Only an explicit Sync now action reaches this prompt. Denial does not
      // stop synchronization; notification visibility is controlled by Android.
      activity.requestPermissions(arrayOf(android.Manifest.permission.POST_NOTIFICATIONS), 4102)
    }
    val timeout = Runnable { failStart("Android did not acknowledge the sync service") }
    synchronized(lock) { SyncPlugin.timeout = timeout }
    handler.postDelayed(timeout, 10_000)
    try {
      val intent = Intent(activity, BevernoteSyncService::class.java).apply {
        action = BevernoteSyncService.ACTION_START
        putExtra(BevernoteSyncService.EXTRA_MESSAGE, args.message)
      }
      if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
        activity.startForegroundService(intent)
      } else {
        activity.startService(intent)
      }
    } catch (error: Exception) {
      failStart(error.message ?: "Cannot start Android sync service")
    }
  }

  @Command
  fun progress(invoke: Invoke) {
    val args = invoke.parseArgs(SyncServiceArgs::class.java)
    BevernoteSyncService.updateNotification(
      activity, args.message, args.current, args.total,
    )
    invoke.resolve()
  }

  @Command
  fun finish(invoke: Invoke) {
    val args = invoke.parseArgs(SyncServiceArgs::class.java)
    BevernoteSyncService.complete(activity, args.message)
    invoke.resolve()
  }
}
