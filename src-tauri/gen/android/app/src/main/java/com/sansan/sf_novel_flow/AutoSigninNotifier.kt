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

    /** 失败提醒按业务日去重的偏好键。 */
    private const val ALERT_DAY_KEY = "auto-signin-alert-day"

    /** 是否已获得通知权限；Android 13 以下恒为已授予。 */
    fun canNotify(context: Context): Boolean =
        Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU ||
            ContextCompat.checkSelfPermission(context, PERMISSION) ==
            PackageManager.PERMISSION_GRANTED

    /** 发一条签到成功通知。 */
    @SuppressLint("MissingPermission")
    fun notifySigned(context: Context, message: String) {
        send(context, "小萝帮您签到成功啦( •̀ ω •́ )✧~", message, android.R.drawable.stat_notify_sync)
    }

    /**
     * 发一条需要用户介入的失败通知，同一业务日最多提醒一次。
     *
     * 周期任务每 2 小时重跑一次，登录失效在用户重新登录前不会自愈，不去重会整天
     * 重复弹。原因未知或拿不到业务日时直接跳过；未授权时静默返回，不记录去重。
     */
    @SuppressLint("MissingPermission")
    fun notifyFailureOnce(context: Context, day: String, reason: String, message: String) {
        val title = when (reason) {
            "login" -> "自动签到失败，请重新登录"
            "risk" -> "自动签到被风控拦截"
            else -> return
        }
        if (day.isBlank()) return
        val prefs = context.getSharedPreferences(DEVICE_PREFS, Context.MODE_PRIVATE)
        if (prefs.getString(ALERT_DAY_KEY, null) == day) return
        send(context, title, message, android.R.drawable.stat_notify_error)
        prefs.edit().putString(ALERT_DAY_KEY, day).apply()
    }

    /** 发送一条签到结果通知；未授权时静默跳过，不抛异常。 */
    @SuppressLint("MissingPermission")
    private fun send(context: Context, title: String, message: String, icon: Int) {
        if (!canNotify(context)) return
        val manager = context.getSystemService(NotificationManager::class.java) ?: return
        ensureChannel(manager)
        val notification = NotificationCompat.Builder(context, CHANNEL_ID)
            .setSmallIcon(icon)
            .setContentTitle(title)
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
