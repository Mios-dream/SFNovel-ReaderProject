package com.sansan.sf_novel_flow

import android.Manifest
import android.app.Activity
import android.content.ActivityNotFoundException
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.os.Environment
import android.provider.DocumentsContract
import android.provider.Settings
import android.webkit.CookieManager
import android.webkit.WebSettings
import androidx.core.content.ContextCompat
import app.tauri.annotation.ActivityCallback
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.Permission
import app.tauri.annotation.PermissionCallback
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.io.File

@InvokeArg
class WriteSessionCookieArgs { lateinit var cookie: String }

@InvokeArg
class WriteExportToUriArgs { lateinit var sourcePath: String; lateinit var uri: String }

@InvokeArg
class SetAutoSigninArgs { var enabled: Boolean = false }

/** Bridges Android authentication state and the official login WebView to Rust. */
@TauriPlugin(
    permissions = [
        Permission(
            strings = [
                Manifest.permission.READ_EXTERNAL_STORAGE,
                Manifest.permission.WRITE_EXTERNAL_STORAGE,
            ],
            alias = "externalStorage",
        ),
    ],
)
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
        invoke.resolve(JSObject().put("cookie", readEncryptedPreference(activity, APP_SESSION_KEY)))
    }

    @Command
    fun writeAppSessionCookie(invoke: Invoke) {
        writeEncryptedPreference(
            activity,
            APP_SESSION_KEY,
            invoke.parseArgs(WriteSessionCookieArgs::class.java).cookie,
        )
        invoke.resolve()
    }

    @Command
    fun readOrCreateDeviceProfile(invoke: Invoke) {
        val profile = readOrCreateDeviceProfile(activity, WebSettings.getDefaultUserAgent(activity))
        invoke.resolve(
            JSObject()
                .put("deviceId", profile.deviceId)
                .put("sdk", profile.sdk)
                .put("abi", profile.abi)
                .put("manufacturer", profile.manufacturer)
                .put("originUa", profile.originUa)
                .put("webViewUA", profile.webViewUa)
        )
    }

    @Command
    fun setAutoSignin(invoke: Invoke) {
        val enabled = invoke.parseArgs(SetAutoSigninArgs::class.java).enabled
        AutoSigninScheduler.sync(activity.applicationContext, enabled)
        invoke.resolve()
    }

    /**
     * 打开本应用的系统详情页。
     *
     * 厂商的自启动页是私有 Activity、名字随系统版本变化，按品牌枚举并不可靠；
     * 系统详情页在所有 Android 上都存在，由用户在其中找到「自启动 / 后台」开关。
     *
     * 是否已授权无法查询（没有 API、没有 ContentProvider），因此这里不做任何校验。
     */
    @Command
    fun openAppSettings(invoke: Invoke) {
        try {
            activity.startActivity(
                Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS)
                    .setData(Uri.parse("package:${activity.packageName}")),
            )
            invoke.resolve()
        } catch (error: Exception) {
            invoke.reject("无法打开应用设置：${error.message}")
        }
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
        if (hasExternalStorageAccess()) {
            invoke.resolve(JSObject().put("granted", true))
            return
        }
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.M) {
            invoke.resolve(JSObject().put("granted", false))
            return
        }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            requestAllFilesAccess(invoke)
        } else {
            requestPermissionForAliases(
                arrayOf("externalStorage"),
                invoke,
                "onExternalStoragePermissionFinished",
            )
        }
    }

    @PermissionCallback
    fun onExternalStoragePermissionFinished(invoke: Invoke) {
        invoke.resolve(JSObject().put("granted", hasExternalStorageAccess()))
    }

    @Suppress("UNUSED_PARAMETER")
    @ActivityCallback
    fun onAllFilesAccessFinished(
        invoke: Invoke,
        _result: androidx.activity.result.ActivityResult,
    ) {
        invoke.resolve(JSObject().put("granted", hasExternalStorageAccess()))
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

    @Command
    fun openDownloadDirectory(invoke: Invoke) {
        try {
            val downloadRoot = Environment.getExternalStorageDirectory()
            val directory = File(downloadRoot, "Download/SF Novel Flow")
            if (!directory.exists() && !directory.mkdirs()) {
                throw java.io.IOException("无法创建下载目录")
            }
            val documentId = "primary:${directory.relativeTo(downloadRoot).invariantSeparatorsPath}"
            val uri = DocumentsContract.buildDocumentUri(EXTERNAL_STORAGE_AUTHORITY, documentId)
            val intent = Intent(Intent.ACTION_VIEW).apply {
                setDataAndType(uri, DocumentsContract.Document.MIME_TYPE_DIR)
                addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_PREFIX_URI_PERMISSION)
            }
            try {
                activity.startActivity(intent)
            } catch (_: ActivityNotFoundException) {
                val picker = Intent(Intent.ACTION_OPEN_DOCUMENT_TREE).apply {
                    putExtra(DocumentsContract.EXTRA_INITIAL_URI, uri)
                    addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_PREFIX_URI_PERMISSION)
                }
                activity.startActivity(picker)
            }
            invoke.resolve()
        } catch (error: Exception) {
            invoke.reject(error.message ?: "无法打开系统文件管理器")
        }
    }

    private fun hasExternalStorageAccess(): Boolean = when {
        Build.VERSION.SDK_INT >= Build.VERSION_CODES.R -> Environment.isExternalStorageManager()
        Build.VERSION.SDK_INT >= Build.VERSION_CODES.M ->
            ContextCompat.checkSelfPermission(
                activity,
                Manifest.permission.WRITE_EXTERNAL_STORAGE,
            ) == PackageManager.PERMISSION_GRANTED
        else -> true
    }

    private fun requestAllFilesAccess(invoke: Invoke) {
        val packageUri = Uri.parse("package:${activity.packageName}")
        val appSettings = Intent(Settings.ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION, packageUri)
        try {
            startActivityForResult(invoke, appSettings, "onAllFilesAccessFinished")
        } catch (_: ActivityNotFoundException) {
            try {
                startActivityForResult(
                    invoke,
                    Intent(Settings.ACTION_MANAGE_ALL_FILES_ACCESS_PERMISSION),
                    "onAllFilesAccessFinished",
                )
            } catch (_: ActivityNotFoundException) {
                invoke.resolve(JSObject().put("granted", false))
            }
        }
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

    private companion object {
        val SFACG_COOKIE_URLS = arrayOf("https://book.sfacg.com/", "https://api.sfacg.com/", "https://m.sfacg.com/login")
        const val SESSION_MAX_AGE = 30 * 24 * 60 * 60
        const val EXTERNAL_STORAGE_AUTHORITY = "com.android.externalstorage.documents"
    }
}
