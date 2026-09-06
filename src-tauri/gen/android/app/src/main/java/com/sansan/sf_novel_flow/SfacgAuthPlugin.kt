package com.sansan.sf_novel_flow

import android.app.Activity
import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.os.Environment
import android.provider.Settings
import android.webkit.CookieManager
import android.util.Base64
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec
import java.util.UUID
import androidx.core.app.ActivityCompat
import androidx.core.content.ContextCompat
import app.tauri.annotation.ActivityCallback
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin

/**
 * Bridges the application-owned Android WebView cookie jar to Rust commands.
 * Cookie values are never returned to JavaScript or serialized to application files.
 *
 * @param activity The foreground Tauri activity used to display the login screen.
 */
@InvokeArg
class WriteSessionCookieArgs {
    lateinit var cookie: String
}

@InvokeArg
class WriteExportToUriArgs {
    lateinit var sourcePath: String
    lateinit var uri: String
}

@TauriPlugin
class SfacgAuthPlugin(private val activity: Activity) : Plugin(activity) {
    /**
     * Opens the isolated official-login activity and resolves after it is closed.
     *
     * @param invoke The native invocation whose result is delivered to Rust only.
     */
    @Command
    fun startOfficialLogin(invoke: Invoke) {
        val intent = Intent(activity, SfacgLoginActivity::class.java)
        startActivityForResult(invoke, intent, "onOfficialLoginFinished")
    }

    /**
     * Resolves the pending Rust command after the login activity exits.
     *
     * @param invoke The original native invocation.
     * @param result The activity result; cancellation is represented by the later status check.
     */
    @ActivityCallback
    fun onOfficialLoginFinished(invoke: Invoke, result: androidx.activity.result.ActivityResult) {
        invoke.resolve(JSObject().put("completed", result.resultCode == Activity.RESULT_OK))
    }

    /**
     * Returns the SF session cookie to Rust without exposing it through the renderer.
     *
     * @param invoke The native invocation receiving the filtered cookie header.
     */
    @Command
    fun readSessionCookie(invoke: Invoke) {
        invoke.resolve(JSObject().put("cookie", readSfacgSessionCookie()))
    }

    /** Writes a native-login cookie into the persistent application WebView jar. */
    @Command
    fun writeSessionCookie(invoke: Invoke) {
        val cookie = invoke.parseArgs(WriteSessionCookieArgs::class.java).cookie
        val manager = CookieManager.getInstance()
        for (part in cookie.split(";")) {
            val pair = part.trim()
            val separator = pair.indexOf('=')
            if (separator <= 0) continue
            val name = pair.substring(0, separator)
            if (name != ".SFCommunity" && name != "session_PC") continue
            for (url in SFACG_COOKIE_URLS) {
                manager.setCookie(url, "$pair; Max-Age=$SESSION_MAX_AGE; Path=/; Secure")
            }
        }
        manager.flush()
        invoke.resolve()
    }

    /** Reads the separately encrypted App session; it is never placed in WebView cookies. */
    @Command
    fun readAppSessionCookie(invoke: Invoke) {
        invoke.resolve(JSObject().put("cookie", readEncryptedPreference(APP_SESSION_KEY)))
    }

    /** Stores the App session using an Android Keystore-backed AES key. */
    @Command
    fun writeAppSessionCookie(invoke: Invoke) {
        val cookie = invoke.parseArgs(WriteSessionCookieArgs::class.java).cookie
        writeEncryptedPreference(APP_SESSION_KEY, cookie)
        invoke.resolve()
    }

    /** Generates one stable UUID per application installation. */
    @Command
    fun readOrCreateDeviceToken(invoke: Invoke) {
        val prefs = activity.getSharedPreferences(DEVICE_PREFS, Activity.MODE_PRIVATE)
        val token = prefs.getString(DEVICE_TOKEN_KEY, null) ?: UUID.randomUUID().toString().uppercase().also {
            prefs.edit().putString(DEVICE_TOKEN_KEY, it).apply()
        }
        invoke.resolve(JSObject().put("token", token))
    }

    /**
     * Deletes the SF session entries from the application WebView cookie jar.
     *
     * @param invoke The native invocation completed after cookies are flushed.
     */
    @Command
    fun clearSessionCookie(invoke: Invoke) {
        val manager = CookieManager.getInstance()
        for (url in SFACG_COOKIE_URLS) {
            manager.setCookie(url, ".SFCommunity=; Max-Age=0; Path=/; Secure")
            manager.setCookie(url, "session_APP=; Max-Age=0; Path=/; Secure")
            manager.setCookie(url, "session_PC=; Max-Age=0; Path=/; Secure")
        }
        activity.getSharedPreferences(DEVICE_PREFS, Activity.MODE_PRIVATE).edit()
            .remove(APP_SESSION_KEY).apply()
        manager.flush()
        invoke.resolve()
    }

    /** Clears only the encrypted App API session without touching website cookies. */
    @Command
    fun clearAppSessionCookie(invoke: Invoke) {
        activity.getSharedPreferences(DEVICE_PREFS, Activity.MODE_PRIVATE).edit()
            .remove(APP_SESSION_KEY).apply()
        invoke.resolve()
    }

    /** Clears only website cookies without touching the separately encrypted App session. */
    @Command
    fun clearWebSessionCookie(invoke: Invoke) {
        val manager = CookieManager.getInstance()
        for (url in SFACG_COOKIE_URLS) {
            manager.setCookie(url, ".SFCommunity=; Max-Age=0; Path=/; Secure")
            manager.setCookie(url, "session_PC=; Max-Age=0; Path=/; Secure")
        }
        manager.flush()
        invoke.resolve()
    }

    /** Checks Android's special shared-storage access and opens its settings page when needed. */
    @Command
    fun ensureExternalStorageAccess(invoke: Invoke) {
        val granted = when {
            Build.VERSION.SDK_INT >= Build.VERSION_CODES.R -> Environment.isExternalStorageManager()
            Build.VERSION.SDK_INT >= Build.VERSION_CODES.M ->
                ContextCompat.checkSelfPermission(activity, Manifest.permission.WRITE_EXTERNAL_STORAGE) ==
                    PackageManager.PERMISSION_GRANTED
            else -> true
        }
        if (!granted && Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            val intent = Intent(
                Settings.ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION,
                Uri.parse("package:${activity.packageName}"),
            )
            activity.startActivity(intent)
        } else if (!granted) {
            ActivityCompat.requestPermissions(
                activity,
                arrayOf(Manifest.permission.WRITE_EXTERNAL_STORAGE),
                STORAGE_PERMISSION_REQUEST_CODE,
            )
        }
        invoke.resolve(JSObject().put("granted", granted))
    }

    /** Copies a generated export into the URI granted by Android's document picker. */
    @Command
    fun writeExportToUri(invoke: Invoke) {
        try {
            val args = invoke.parseArgs(WriteExportToUriArgs::class.java)
            val target = Uri.parse(args.uri)
            val resolver = activity.contentResolver
            resolver.openOutputStream(target, "w")?.use { output ->
                java.io.FileInputStream(args.sourcePath).use { input -> input.copyTo(output) }
            } ?: throw java.io.IOException("无法打开系统保存位置")
            invoke.resolve()
        } catch (error: Exception) {
            invoke.reject(error.message ?: "无法写入系统保存位置")
        }
    }

    /**
     * Reads and filters cookies required by SF requests from the app-owned WebView jar.
     *
     * @returns A de-duplicated request header or null when no recognized session exists.
     */
    private fun readSfacgSessionCookie(): String? {
        val cookiePairs = linkedMapOf<String, String>()
        val manager = CookieManager.getInstance()
        for (url in SFACG_COOKIE_URLS) {
            val header = manager.getCookie(url) ?: continue
            for (part in header.split(";")) {
                val pair = part.trim()
                val separator = pair.indexOf('=')
                if (separator <= 0) continue
                val name = pair.substring(0, separator)
                if (name == ".SFCommunity" || name == "session_PC") {
                    cookiePairs[name] = pair.substring(separator + 1)
                }
            }
        }
        if (!cookiePairs.containsKey(".SFCommunity")) return null
        return cookiePairs.entries.joinToString("; ") { (name, value) -> "$name=$value" }
    }

    private companion object {
        /** SF domains whose application WebView cookies can be used by native requests. */
        val SFACG_COOKIE_URLS = arrayOf(
            "https://book.sfacg.com/",
            "https://api.sfacg.com/",
            "https://passport.sfacg.com/",
        )
        const val SESSION_MAX_AGE = 30 * 24 * 60 * 60
        const val STORAGE_PERMISSION_REQUEST_CODE = 4101
        const val DEVICE_PREFS = "sfacg-device"
        const val DEVICE_TOKEN_KEY = "device-token"
        const val APP_SESSION_KEY = "app-session"
        const val KEY_ALIAS = "sfacg-auth-key"
    }

    private fun getKeystoreKey(): SecretKey {
        val keyStore = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        (keyStore.getKey(KEY_ALIAS, null) as? SecretKey)?.let { return it }
        val generator = KeyGenerator.getInstance("AES", "AndroidKeyStore")
        generator.init(android.security.keystore.KeyGenParameterSpec.Builder(
            KEY_ALIAS,
            android.security.keystore.KeyProperties.PURPOSE_ENCRYPT or
                android.security.keystore.KeyProperties.PURPOSE_DECRYPT,
        ).setBlockModes(android.security.keystore.KeyProperties.BLOCK_MODE_GCM)
            .setEncryptionPaddings(android.security.keystore.KeyProperties.ENCRYPTION_PADDING_NONE)
            .build())
        return generator.generateKey()
    }

    private fun writeEncryptedPreference(key: String, value: String) {
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        cipher.init(Cipher.ENCRYPT_MODE, getKeystoreKey())
        val encoded = Base64.encodeToString(cipher.iv + cipher.doFinal(value.toByteArray(Charsets.UTF_8)), Base64.NO_WRAP)
        activity.getSharedPreferences(DEVICE_PREFS, Activity.MODE_PRIVATE).edit().putString(key, encoded).apply()
    }

    private fun readEncryptedPreference(key: String): String? {
        val encoded = activity.getSharedPreferences(DEVICE_PREFS, Activity.MODE_PRIVATE).getString(key, null) ?: return null
        return try {
            val payload = Base64.decode(encoded, Base64.NO_WRAP)
            val cipher = Cipher.getInstance("AES/GCM/NoPadding")
            cipher.init(Cipher.DECRYPT_MODE, getKeystoreKey(), GCMParameterSpec(128, payload.copyOfRange(0, 12)))
            String(cipher.doFinal(payload.copyOfRange(12, payload.size)), Charsets.UTF_8)
        } catch (_: Exception) {
            null
        }
    }

}
