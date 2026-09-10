package com.sansan.sf_novel_flow

import android.annotation.SuppressLint
import android.app.Activity
import android.os.Bundle
import android.view.View
import android.view.ViewGroup
import android.webkit.CookieManager
import android.webkit.WebResourceRequest
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.appcompat.app.AppCompatActivity

/**
 * Hosts the SF official login page in an application-owned WebView.
 * The activity never reads passwords or injects scripts into the login page.
 */
class SfacgLoginActivity : AppCompatActivity() {
    private lateinit var webView: WebView

    /**
     * Creates a WebView restricted to HTTPS navigations and starts the official login flow.
     *
     * @param savedInstanceState Android state supplied during activity recreation.
     */
    @SuppressLint("SetJavaScriptEnabled")
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        CookieManager.getInstance().setAcceptCookie(true)
        webView = WebView(this).apply {
            overScrollMode = View.OVER_SCROLL_NEVER
            isVerticalScrollBarEnabled = false
            isHorizontalScrollBarEnabled = false
            settings.javaScriptEnabled = true
            settings.domStorageEnabled = true
            settings.javaScriptCanOpenWindowsAutomatically = false
            settings.setSupportMultipleWindows(false)
            webViewClient = object : WebViewClient() {
                /**
                 * Blocks non-HTTPS navigation so a login page cannot open local app resources.
                 *
                 * @param view The WebView requesting navigation.
                 * @param request The requested destination.
                 * @returns True when the navigation is blocked.
                 */
                override fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest): Boolean {
                    return request.url.scheme != "https"
                }

                /**
                 * Closes the activity after SF has placed its authenticated WebView cookie.
                 *
                 * @param view The WebView that finished loading.
                 * @param url The page URL that has completed loading.
                 */
                override fun onPageFinished(view: WebView, url: String) {
                    super.onPageFinished(view, url)
                    finishWhenSessionCookieExists(url)
                }
            }
        }
        setContentView(webView, ViewGroup.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT,
            ViewGroup.LayoutParams.MATCH_PARENT,
        ))
        webView.loadUrl(OFFICIAL_LOGIN_URL)
    }

    /**
     * Finishes only after an SF domain has issued the primary authenticated session cookie.
     *
     * @param url The recently loaded URL used to prevent unrelated pages from completing login.
     */
    private fun finishWhenSessionCookieExists(url: String) {
        val host = android.net.Uri.parse(url).host?.lowercase() ?: return
        if (host != "sfacg.com" && !host.endsWith(".sfacg.com")) return
        val cookie = CookieManager.getInstance().getCookie("https://book.sfacg.com/") ?: return
        if (!cookie.split(";").any { it.trim().startsWith(".SFCommunity=") }) return
        CookieManager.getInstance().flush()
        setResult(Activity.RESULT_OK)
        finish()
    }

    /** Releases the WebView promptly when the login screen is closed. */
    override fun onDestroy() {
        if (::webView.isInitialized) {
            webView.stopLoading()
            webView.destroy()
        }
        super.onDestroy()
    }

    private companion object {
        /** The SF-hosted entrypoint where users complete their own authentication. */
        const val OFFICIAL_LOGIN_URL = "https://passport.sfacg.com/"
    }
}
