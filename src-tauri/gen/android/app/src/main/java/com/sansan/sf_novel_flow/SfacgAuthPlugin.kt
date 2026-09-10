package com.sansan.sf_novel_flow

import android.Manifest
import android.app.Activity
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.os.Environment
import android.provider.Settings
import android.util.Base64
import android.webkit.CookieManager
import androidx.core.app.ActivityCompat
import androidx.core.content.ContextCompat
import app.tauri.annotation.ActivityCallback
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.security.KeyStore
import java.util.UUID
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

@InvokeArg
class WriteSessionCookieArgs { lateinit var cookie: String }

@InvokeArg
class WriteExportToUriArgs { lateinit var sourcePath: String; lateinit var uri: String }

/** Bridges Android authentication state and the official login WebView to Rust. */
@TauriPlugin
class SfacgAuthPlugin(private val activity: Activity) : Plugin(activity) {
    @Command
    fun startOfficialLogin(invoke: Invoke) {
        val intent = Intent(activity, SfacgLoginActivity::class.java)
        startActivityForResult(invoke, intent, "onOfficialLoginFinished")
    }

    @ActivityCallback
    fun onOfficialLoginFinished(invoke: Invoke, result: androidx.activity.result.ActivityResult) {
        invoke.resolve(JSObject().put("completed", result.resultCode == Activity.RESULT_OK))
    }

    @Command
    fun readSessionCookie(invoke: Invoke) {
        invoke.resolve(JSObject().put("cookie", readSfacgSessionCookie()))
    }

    @Command
    fun writeSessionCookie(invoke: Invoke) {
        val cookie = invoke.parseArgs(WriteSessionCookieArgs::class.java).cookie
        val manager = CookieManager.getInstance()
        for (part in cookie.split(";")) {
            val pair = part.trim(); val separator = pair.indexOf('=')
            if (separator <= 0) continue
            val name = pair.substring(0, separator)
            if (name != ".SFCommunity" && name != "session_PC") continue
            for (url in SFACG_COOKIE_URLS) manager.setCookie(url, "$pair; Max-Age=$SESSION_MAX_AGE; Path=/; Secure")
        }
        manager.flush(); invoke.resolve()
    }

    @Command
    fun readAppSessionCookie(invoke: Invoke) {
        invoke.resolve(JSObject().put("cookie", readEncryptedPreference(APP_SESSION_KEY)))
    }

    @Command
    fun writeAppSessionCookie(invoke: Invoke) {
        writeEncryptedPreference(APP_SESSION_KEY, invoke.parseArgs(WriteSessionCookieArgs::class.java).cookie)
        invoke.resolve()
    }

    @Command
    fun readOrCreateDeviceToken(invoke: Invoke) {
        val prefs = activity.getSharedPreferences(DEVICE_PREFS, Activity.MODE_PRIVATE)
        val token = prefs.getString(DEVICE_TOKEN_KEY, null) ?: UUID.randomUUID().toString().uppercase().also {
            prefs.edit().putString(DEVICE_TOKEN_KEY, it).apply()
        }
        invoke.resolve(JSObject().put("token", token))
    }

    @Command
    fun clearSessionCookie(invoke: Invoke) {
        val manager = CookieManager.getInstance()
        for (url in SFACG_COOKIE_URLS) {
            manager.setCookie(url, ".SFCommunity=; Max-Age=0; Path=/; Secure")
            manager.setCookie(url, "session_APP=; Max-Age=0; Path=/; Secure")
            manager.setCookie(url, "session_PC=; Max-Age=0; Path=/; Secure")
        }
        activity.getSharedPreferences(DEVICE_PREFS, Activity.MODE_PRIVATE).edit().remove(APP_SESSION_KEY).apply()
        manager.flush(); invoke.resolve()
    }

    @Command
    fun clearAppSessionCookie(invoke: Invoke) {
        activity.getSharedPreferences(DEVICE_PREFS, Activity.MODE_PRIVATE).edit().remove(APP_SESSION_KEY).apply()
        invoke.resolve()
    }

    @Command
    fun clearWebSessionCookie(invoke: Invoke) {
        val manager = CookieManager.getInstance()
        for (url in SFACG_COOKIE_URLS) {
            manager.setCookie(url, ".SFCommunity=; Max-Age=0; Path=/; Secure")
            manager.setCookie(url, "session_PC=; Max-Age=0; Path=/; Secure")
        }
        manager.flush(); invoke.resolve()
    }

    @Command
    fun ensureExternalStorageAccess(invoke: Invoke) {
        val granted = when {
            Build.VERSION.SDK_INT >= Build.VERSION_CODES.R -> Environment.isExternalStorageManager()
            Build.VERSION.SDK_INT >= Build.VERSION_CODES.M -> ContextCompat.checkSelfPermission(activity, Manifest.permission.WRITE_EXTERNAL_STORAGE) == PackageManager.PERMISSION_GRANTED
            else -> true
        }
        if (!granted && Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            activity.startActivity(Intent(Settings.ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION, Uri.parse("package:${activity.packageName}")))
        } else if (!granted) {
            ActivityCompat.requestPermissions(activity, arrayOf(Manifest.permission.WRITE_EXTERNAL_STORAGE), STORAGE_PERMISSION_REQUEST_CODE)
        }
        invoke.resolve(JSObject().put("granted", granted))
    }

    @Command
    fun writeExportToUri(invoke: Invoke) {
        try {
            val args = invoke.parseArgs(WriteExportToUriArgs::class.java)
            val target = Uri.parse(args.uri)
            activity.contentResolver.openOutputStream(target, "w")?.use { output ->
                java.io.FileInputStream(args.sourcePath).use { input -> input.copyTo(output) }
            } ?: throw java.io.IOException("无法打开系统保存位置")
            invoke.resolve()
        } catch (error: Exception) { invoke.reject(error.message ?: "无法写入系统保存位置") }
    }

    private fun readSfacgSessionCookie(): String? {
        val cookiePairs = linkedMapOf<String, String>(); val manager = CookieManager.getInstance()
        for (url in SFACG_COOKIE_URLS) for (part in (manager.getCookie(url) ?: "").split(";")) {
            val pair = part.trim(); val separator = pair.indexOf('=')
            if (separator <= 0) continue
            val name = pair.substring(0, separator)
            if (name == ".SFCommunity" || name == "session_PC") cookiePairs[name] = pair.substring(separator + 1)
        }
        if (!cookiePairs.containsKey(".SFCommunity")) return null
        return cookiePairs.entries.joinToString("; ") { (name, value) -> "$name=$value" }
    }

    private fun getKeystoreKey(): SecretKey {
        val keyStore = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        (keyStore.getKey(KEY_ALIAS, null) as? SecretKey)?.let { return it }
        val generator = KeyGenerator.getInstance("AES", "AndroidKeyStore")
        generator.init(android.security.keystore.KeyGenParameterSpec.Builder(KEY_ALIAS, android.security.keystore.KeyProperties.PURPOSE_ENCRYPT or android.security.keystore.KeyProperties.PURPOSE_DECRYPT)
            .setBlockModes(android.security.keystore.KeyProperties.BLOCK_MODE_GCM)
            .setEncryptionPaddings(android.security.keystore.KeyProperties.ENCRYPTION_PADDING_NONE).build())
        return generator.generateKey()
    }

    private fun writeEncryptedPreference(key: String, value: String) {
        val cipher = Cipher.getInstance("AES/GCM/NoPadding"); cipher.init(Cipher.ENCRYPT_MODE, getKeystoreKey())
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
        } catch (_: Exception) { null }
    }

    private companion object {
        val SFACG_COOKIE_URLS = arrayOf("https://book.sfacg.com/", "https://api.sfacg.com/", "https://m.sfacg.com/login")
        const val SESSION_MAX_AGE = 30 * 24 * 60 * 60
        const val STORAGE_PERMISSION_REQUEST_CODE = 4101
        const val DEVICE_PREFS = "sfacg-device"
        const val DEVICE_TOKEN_KEY = "device-token"
        const val APP_SESSION_KEY = "app-session"
        const val KEY_ALIAS = "sfacg-auth-key"
    }
}
