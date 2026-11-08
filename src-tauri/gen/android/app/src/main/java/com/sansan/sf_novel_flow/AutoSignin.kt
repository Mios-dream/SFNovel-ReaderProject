package com.sansan.sf_novel_flow

import android.content.Context
import android.os.Build
import android.util.Base64
import android.util.Log
import androidx.work.BackoffPolicy
import androidx.work.Constraints
import androidx.work.Data
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.ExistingWorkPolicy
import androidx.work.NetworkType
import androidx.work.OneTimeWorkRequest
import androidx.work.PeriodicWorkRequest
import androidx.work.WorkManager
import androidx.work.Worker
import androidx.work.WorkerParameters
import org.json.JSONObject
import java.security.KeyStore
import java.util.Locale
import java.util.UUID
import java.util.concurrent.TimeUnit
import java.util.concurrent.locks.ReentrantLock
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

internal const val DEVICE_PREFS = "sfacg-device"
internal const val DEVICE_ID_KEY = "device-id"
internal const val APP_SESSION_KEY = "app-session"
private const val DEVICE_PROFILE_KEY = "device-profile"
private const val KEY_ALIAS = "sfacg-auth-key"

internal data class AndroidDeviceProfile(
    val deviceId: String,
    val sdk: Int,
    val abi: String,
    val manufacturer: String,
    val originUa: String,
    val webViewUa: String,
) {
    fun toJson(): String = JSONObject()
        .put("deviceId", deviceId)
        .put("sdk", sdk)
        .put("abi", abi)
        .put("manufacturer", manufacturer)
        .put("originUa", originUa)
        .put("webViewUA", webViewUa)
        .toString()
}

internal fun readOrCreateDeviceProfile(context: Context, webViewUa: String): AndroidDeviceProfile {
    val prefs = context.getSharedPreferences(DEVICE_PREFS, Context.MODE_PRIVATE)
    val deviceId = prefs.getString(DEVICE_ID_KEY, null) ?: UUID.randomUUID().toString().lowercase().also {
        prefs.edit().putString(DEVICE_ID_KEY, it).apply()
    }
    val profile = AndroidDeviceProfile(
        deviceId = deviceId,
        sdk = Build.VERSION.SDK_INT,
        abi = Build.SUPPORTED_ABIS.firstOrNull() ?: "",
        manufacturer = Build.MANUFACTURER.lowercase(Locale.ROOT),
        originUa = System.getProperty("http.agent") ?: "",
        webViewUa = webViewUa,
    )
    prefs.edit().putString(DEVICE_PROFILE_KEY, profile.toJson()).apply()
    return profile
}

private fun readCachedDeviceProfile(context: Context): AndroidDeviceProfile? {
    val encoded = context.getSharedPreferences(DEVICE_PREFS, Context.MODE_PRIVATE)
        .getString(DEVICE_PROFILE_KEY, null) ?: return null
    return try {
        val value = JSONObject(encoded)
        AndroidDeviceProfile(
            deviceId = value.getString("deviceId"),
            sdk = value.getInt("sdk"),
            abi = value.getString("abi"),
            manufacturer = value.getString("manufacturer"),
            originUa = value.getString("originUa"),
            webViewUa = value.getString("webViewUA"),
        )
    } catch (_: Exception) {
        null
    }
}

private fun getKeystoreKey(): SecretKey {
    val keyStore = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
    (keyStore.getKey(KEY_ALIAS, null) as? SecretKey)?.let { return it }
    val generator = KeyGenerator.getInstance("AES", "AndroidKeyStore")
    generator.init(
        android.security.keystore.KeyGenParameterSpec.Builder(
            KEY_ALIAS,
            android.security.keystore.KeyProperties.PURPOSE_ENCRYPT or
                android.security.keystore.KeyProperties.PURPOSE_DECRYPT,
        )
            .setBlockModes(android.security.keystore.KeyProperties.BLOCK_MODE_GCM)
            .setEncryptionPaddings(android.security.keystore.KeyProperties.ENCRYPTION_PADDING_NONE)
            .build(),
    )
    return generator.generateKey()
}

internal fun writeEncryptedPreference(context: Context, key: String, value: String) {
    val cipher = Cipher.getInstance("AES/GCM/NoPadding")
    cipher.init(Cipher.ENCRYPT_MODE, getKeystoreKey())
    val encoded = Base64.encodeToString(
        cipher.iv + cipher.doFinal(value.toByteArray(Charsets.UTF_8)),
        Base64.NO_WRAP,
    )
    context.getSharedPreferences(DEVICE_PREFS, Context.MODE_PRIVATE)
        .edit().putString(key, encoded).apply()
}

internal fun readEncryptedPreference(context: Context, key: String): String? {
    val encoded = context.getSharedPreferences(DEVICE_PREFS, Context.MODE_PRIVATE)
        .getString(key, null) ?: return null
    return try {
        val payload = Base64.decode(encoded, Base64.NO_WRAP)
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        cipher.init(
            Cipher.DECRYPT_MODE,
            getKeystoreKey(),
            GCMParameterSpec(128, payload.copyOfRange(0, 12)),
        )
        String(cipher.doFinal(payload.copyOfRange(12, payload.size)), Charsets.UTF_8)
    } catch (_: Exception) {
        null
    }
}

/** 一次性检查任务的输入标记：失败时直接结束，由周期任务兜底重试。 */
private const val ONE_SHOT_KEY = "one-shot"

/**
 * 串行化签到提交。
 *
 * 周期任务与启动检查可能同时被唤醒。并发提交时后一次会拿到"已签到"的上游错误，
 * 并把它写进本地记录覆盖前一次的成功结果，用户随后会看到一条假失败提示。
 * 拿不到锁即说明另一条正在处理，本次直接结束。
 */
private val SIGNIN_GATE = ReentrantLock()

internal object AutoSigninBridge {
    init {
        System.loadLibrary("sf_novel_flow_lib")
    }

    external fun run(dataDir: String, cookie: String, deviceJson: String): String
}

class AutoSigninWorker(context: Context, params: WorkerParameters) : Worker(context, params) {
    override fun doWork(): Result {
        if (!SIGNIN_GATE.tryLock()) {
            Log.i(LOG_TAG, "已有检查在执行，跳过本次")
            return Result.success()
        }
        return try {
            check()
        } finally {
            SIGNIN_GATE.unlock()
        }
    }

    private fun check(): Result {
        val context = applicationContext
        val cookie = readEncryptedPreference(context, APP_SESSION_KEY)
        if (cookie.isNullOrBlank()) {
            Log.i(LOG_TAG, "未登录，跳过自动签到")
            return Result.success()
        }
        val profile = readCachedDeviceProfile(context)
            ?: readOrCreateDeviceProfile(context, System.getProperty("http.agent") ?: "")
        return try {
            val outcome = JSONObject(
                AutoSigninBridge.run(
                    context.dataDir.absolutePath,
                    cookie,
                    profile.toJson(),
                ),
            )
            Log.i(LOG_TAG, outcome.optString("message"))
            if (outcome.optBoolean("retry") && !isOneShot()) Result.retry() else Result.success()
        } catch (error: Throwable) {
            Log.w(LOG_TAG, "自动签到失败", error)
            if (isOneShot()) Result.success() else Result.retry()
        }
    }

    /**
     * 本次是否为启动时的一次性检查。
     *
     * 一次性任务的唯一名会被下一次启动复用，若它带着退避一直重试，
     * 后续启动的 `checkNow` 就被 KEEP 策略挡在外面，反而更晚才检查。
     */
    private fun isOneShot(): Boolean = inputData.getBoolean(ONE_SHOT_KEY, false)

    private companion object {
        const val LOG_TAG = "AutoSignin"
    }
}

internal object AutoSigninScheduler {
    private const val WORK_NAME = "auto-signin-daily"
    private const val CHECK_WORK_NAME = "auto-signin-check"
    private const val CHECK_INTERVAL_HOURS = 2L
    private const val RETRY_BACKOFF_MINUTES = 30L
    private const val ENABLED_KEY = "auto-signin-enabled"

    /**
     * 保存开关状态并按需重排任务。
     *
     * 周期任务本身会重试，因此这里用 UPDATE 只更新参数，不清除已有运行历史；
     * 启动时也调用一次，修复升级、清数据或强制停止后丢失的调度。
     *
     * 首次延迟为 0：关闭开关时已经取消任务，重新打开会新建一条周期任务，
     * 这条任务在联网条件满足后立即执行一次，用户不必等满一个周期。
     */
    fun sync(context: Context, enabled: Boolean) {
        context.getSharedPreferences(DEVICE_PREFS, Context.MODE_PRIVATE)
            .edit().putBoolean(ENABLED_KEY, enabled).apply()
        val manager = WorkManager.getInstance(context)
        if (!enabled) {
            manager.cancelUniqueWork(WORK_NAME)
            return
        }
        val request = PeriodicWorkRequest.Builder(
            AutoSigninWorker::class.java,
            CHECK_INTERVAL_HOURS,
            TimeUnit.HOURS,
        )
            .setInitialDelay(0, TimeUnit.SECONDS)
            .setConstraints(networkConstraint())
            .setBackoffCriteria(
                BackoffPolicy.EXPONENTIAL,
                RETRY_BACKOFF_MINUTES,
                TimeUnit.MINUTES,
            )
            .build()
        manager.enqueueUniquePeriodicWork(WORK_NAME, ExistingPeriodicWorkPolicy.UPDATE, request)
    }

    /**
     * 立刻检查一次，不等周期。
     *
     * 应用被系统强制停止后，JobScheduler 里的周期任务会被一并清空。在用户重新打开
     * 应用之前，JobScheduler、AlarmManager 与开机广播都不会送达这个包，任何后台
     * 调度都不存在——这是平台行为，换调度方案绕不过去。
     *
     * 因此把"打开应用"做成可靠的恢复点：已签过就由 Rust 侧读本地记录直接返回，
     * 不发起请求；未签则当场提交，不必再等近一个周期。
     */
    fun checkNow(context: Context) {
        val enabled = context.getSharedPreferences(DEVICE_PREFS, Context.MODE_PRIVATE)
            .getBoolean(ENABLED_KEY, false)
        if (!enabled) return
        val request = OneTimeWorkRequest.Builder(AutoSigninWorker::class.java)
            .setInputData(Data.Builder().putBoolean(ONE_SHOT_KEY, true).build())
            .setConstraints(networkConstraint())
            .build()
        WorkManager.getInstance(context)
            .enqueueUniqueWork(CHECK_WORK_NAME, ExistingWorkPolicy.KEEP, request)
    }

    /** 按上次保存的开关状态恢复调度。 */
    fun restore(context: Context) {
        val enabled = context.getSharedPreferences(DEVICE_PREFS, Context.MODE_PRIVATE)
            .getBoolean(ENABLED_KEY, false)
        sync(context, enabled)
    }

    /** 周期任务与一次性检查共用的联网约束。 */
    private fun networkConstraint(): Constraints = Constraints.Builder()
        .setRequiredNetworkType(NetworkType.CONNECTED)
        .build()
}
