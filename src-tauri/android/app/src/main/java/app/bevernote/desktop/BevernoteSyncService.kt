package app.bevernote.desktop

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.IBinder

/**
 * Keeps this application's existing Rust worker alive while its Activity is
 * gone. The service never starts a second engine or restores work after death.
 */
class BevernoteSyncService : Service() {
  companion object {
    const val ACTION_START = "app.bevernote.desktop.sync.START"
    const val EXTRA_MESSAGE = "message"
    private const val CHANNEL_ID = "pcloud-sync"
    private const val NOTIFICATION_ID = 4102
    private var instance: BevernoteSyncService? = null
    private var lastNotificationAt = 0L

    fun updateNotification(context: Context, message: String, current: Long, total: Long) {
      val service = instance ?: return
      val now = System.currentTimeMillis()
      if (now - lastNotificationAt < 1_000) return
      lastNotificationAt = now
      service.showProgress(message, current, total)
    }

    fun complete(context: Context, message: String) {
      val manager = context.getSystemService(NotificationManager::class.java)
      manager.notify(
        NOTIFICATION_ID + 1,
        notification(context, message, 0, 0, ongoing = false),
      )
      instance?.stopForeground(STOP_FOREGROUND_REMOVE)
      instance?.stopSelf()
      instance = null
      manager.cancel(NOTIFICATION_ID)
    }

    private fun notification(
      context: Context,
      message: String,
      current: Long,
      total: Long,
      ongoing: Boolean,
    ): Notification {
      val launch = context.packageManager.getLaunchIntentForPackage(context.packageName)
      val pendingIntent = PendingIntent.getActivity(
        context,
        0,
        launch,
        PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
      )
      val builder = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
        Notification.Builder(context, CHANNEL_ID)
      } else {
        @Suppress("DEPRECATION") Notification.Builder(context)
      }
        .setSmallIcon(R.drawable.ic_sync)
        .setContentTitle("Bevernote sync")
        .setContentText(message)
        .setStyle(Notification.BigTextStyle().bigText(message))
        .setContentIntent(pendingIntent)
        .setOnlyAlertOnce(true)
        .setOngoing(ongoing)
        .setAutoCancel(!ongoing)
        .setCategory(Notification.CATEGORY_PROGRESS)
        .setVisibility(Notification.VISIBILITY_PRIVATE)
      if (total > 0) {
        builder.setProgress(total.toInt(), current.toInt().coerceIn(0, total.toInt()), false)
      } else {
        builder.setProgress(0, 0, ongoing)
      }
      return builder.build()
    }
  }

  override fun onCreate() {
    super.onCreate()
    val manager = getSystemService(NotificationManager::class.java)
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
      manager.createNotificationChannel(
        NotificationChannel(CHANNEL_ID, "pCloud sync", NotificationManager.IMPORTANCE_LOW),
      )
    }
  }

  override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
    if (intent?.action != ACTION_START) return START_NOT_STICKY
    instance = this
    try {
      val notification = notification(
        this,
        intent.getStringExtra(EXTRA_MESSAGE) ?: "Synchronizing…",
        0,
        0,
        ongoing = true,
      )
      if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
        startForeground(NOTIFICATION_ID, notification, ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC)
      } else {
        startForeground(NOTIFICATION_ID, notification)
      }
      SyncPlugin.acknowledgeStart()
    } catch (error: Exception) {
      SyncPlugin.failStart(error.message ?: "Cannot display Android sync notification")
      stopForeground(STOP_FOREGROUND_REMOVE)
      stopSelf()
      instance = null
    }
    // An OS process restart cannot infer credentials or resume an unfinished
    // transaction safely, so it must never create a new synchronization run.
    return START_NOT_STICKY
  }

  private fun showProgress(message: String, current: Long, total: Long) {
    getSystemService(NotificationManager::class.java).notify(
      NOTIFICATION_ID,
      notification(this, message, current, total, ongoing = true),
    )
  }

  override fun onDestroy() {
    if (instance === this) instance = null
    super.onDestroy()
  }

  override fun onBind(intent: Intent?): IBinder? = null
}
