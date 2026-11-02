package com.sansan.sf_novel_flow

import android.content.Context
import android.os.Build
import android.util.Base64
import android.util.Log
import androidx.work.Constraints
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.NetworkType
import androidx.work.PeriodicWorkRequest
import androidx.work.WorkManager
import androidx.work.Worker
import androidx.work.WorkerParameters
import org.json.JSONObject
import java.security.KeyStore
import java.util.Calendar
import java.util.Locale
import java.util.TimeZone
import java.util.UUID
import java.util.concurrent.TimeUnit
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

internal const val DEVICE_PREFS = "sfacg-device"
internal const val DEVICE_ID_KEY = "device-id"
internal const val APP_SESSION_KEY = "app-session"
private const val DEVICE_PROFILE_KEY = "device-profile"
private const val KEY_ALIAS = "sfacg-auth-key"
private const val BEIJING_TIME_ZONE = "Asia/Shanghai"

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

internal object AutoSigninBridge {
    init {
        System.loadLibrary("sf_novel_flow_lib")
    }

    external fun run(dataDir: String, cookie: String, deviceJson: String, date: String): String
}

class AutoSigninWorker(context: Context, params: WorkerParameters) : Worker(context, params) {
    override fun doWork(): Result {
        val context = applicationContext
        return try {
            val cookie = readEncryptedPreference(context, APP_SESSION_KEY)
            if (cookie.isNullOrBlank()) {
                Log.i(LOG_TAG, "未登录，跳过自动签到")
                return Result.success()
            }
            val profile = readCachedDeviceProfile(context)
                ?: readOrCreateDeviceProfile(context, System.getProperty("http.agent") ?: "")
            val message = AutoSigninBridge.run(
                context.dataDir.absolutePath,
                cookie,
                profile.toJson(),
                beijingDate(),
            )
            Log.i(LOG_TAG, message)
            Result.success()
        } catch (error: Throwable) {
            Log.w(LOG_TAG, "自动签到失败", error)
            Result.success()
        }
    }

    private companion object {
        const val LOG_TAG = "AutoSignin"
    }
}

internal object AutoSigninScheduler {
    private const val WORK_NAME = "auto-signin-daily"

    fun schedule(context: Context) {
        val request = PeriodicWorkRequest.Builder(AutoSigninWorker::class.java, 24, TimeUnit.HOURS)
            .setInitialDelay(delayUntilNextNoon(), TimeUnit.MILLISECONDS)
            .setConstraints(
                Constraints.Builder()
                    .setRequiredNetworkType(NetworkType.CONNECTED)
                    .build(),
            )
            .build()
        WorkManager.getInstance(context).enqueueUniquePeriodicWork(
            WORK_NAME,
            ExistingPeriodicWorkPolicy.CANCEL_AND_REENQUEUE,
            request,
        )
    }

    fun cancel(context: Context) {
        WorkManager.getInstance(context).cancelUniqueWork(WORK_NAME)
    }

    private fun delayUntilNextNoon(): Long {
        val zone = TimeZone.getTimeZone(BEIJING_TIME_ZONE)
        val now = Calendar.getInstance(zone)
        val next = now.clone() as Calendar
        next.set(Calendar.HOUR_OF_DAY, 12)
        next.set(Calendar.MINUTE, 0)
        next.set(Calendar.SECOND, 0)
        next.set(Calendar.MILLISECOND, 0)
        if (!next.after(now)) next.add(Calendar.DAY_OF_YEAR, 1)
        return next.timeInMillis - now.timeInMillis
    }
}

private fun beijingDate(): String {
    val now = Calendar.getInstance(TimeZone.getTimeZone(BEIJING_TIME_ZONE))
    return String.format(
        Locale.ROOT,
        "%04d-%02d-%02d",
        now.get(Calendar.YEAR),
        now.get(Calendar.MONTH) + 1,
        now.get(Calendar.DAY_OF_MONTH),
    )
}
