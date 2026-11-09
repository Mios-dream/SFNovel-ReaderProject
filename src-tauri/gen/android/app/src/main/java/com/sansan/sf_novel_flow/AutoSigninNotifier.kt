package com.sansan.sf_novel_flow

import android.Manifest
import android.annotation.SuppressLint
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import androidx.core.content.ContextCompat

/**
 * 自动签到结果通知。
 *
 * 后台执行时用户看不到应用内提示，系统通知是唯一能让用户知道「今天签到了」的渠道。
 * 使用系统接口：Android 8 起通知必须归属渠道，Android 13 起还需要运行时授权。
 */
internal object AutoSigninNotifier {
    private const val CHANNEL_ID = "auto-signin"
    private const val NOTIFICATION_ID = 1001
    private const val PERMISSION = Manifest.permission.POST_NOTIFICATIONS

    /** 是否已获得通知权限；Android 13 以下恒为已授予。 */
    fun canNotify(context: Context): Boolean =
        Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU ||
            ContextCompat.checkSelfPermission(context, PERMISSION) ==
            PackageManager.PERMISSION_GRANTED

    /** 发一条签到成功通知；未授权时静默跳过，不抛异常。 */
    @SuppressLint("MissingPermission")
    fun notifySigned(context: Context, message: String) {
        if (!canNotify(context)) return
        val manager = context.getSystemService(NotificationManager::class.java) ?: return
        ensureChannel(manager)
        val notification = NotificationCompat.Builder(context, CHANNEL_ID)
            .setSmallIcon(android.R.drawable.stat_notify_sync)
            .setContentTitle("自动签到成功")
            .setContentText(message)
            .setStyle(NotificationCompat.BigTextStyle().bigText(message))
            .setPriority(NotificationCompat.PRIORITY_LOW)
            .setAutoCancel(true)
            .setContentIntent(openAppIntent(context))
            .build()
        NotificationManagerCompat.from(context).notify(NOTIFICATION_ID, notification)
    }

    /** Android 8 起通知必须归属渠道；重复创建同名渠道无副作用。 */
    private fun ensureChannel(manager: NotificationManager) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return
        if (manager.getNotificationChannel(CHANNEL_ID) != null) return
        // 签到结果只需知晓、不需立刻处理，用低重要性避免夜间响铃。
        manager.createNotificationChannel(
            NotificationChannel(CHANNEL_ID, "自动签到", NotificationManager.IMPORTANCE_LOW)
                .apply { description = "每日自动签到结果" },
        )
    }

    /** 点击通知回到应用。 */
    private fun openAppIntent(context: Context): PendingIntent = PendingIntent.getActivity(
        context,
        0,
        Intent(context, MainActivity::class.java)
            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TOP),
        PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
    )
}
