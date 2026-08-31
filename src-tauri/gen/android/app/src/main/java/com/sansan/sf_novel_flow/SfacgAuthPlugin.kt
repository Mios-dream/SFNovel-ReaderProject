package com.sansan.sf_novel_flow

import android.app.Activity
import android.content.Intent
import android.webkit.CookieManager
import app.tauri.annotation.ActivityCallback
import app.tauri.annotation.Command
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
        }
        manager.flush()
        invoke.resolve()
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
                if (name == ".SFCommunity" || name == "session_APP" || name.startsWith("session_")) {
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
    }
}
