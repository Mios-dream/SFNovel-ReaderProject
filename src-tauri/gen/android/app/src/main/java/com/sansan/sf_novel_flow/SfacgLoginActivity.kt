package com.sansan.sf_novel_flow

import android.annotation.SuppressLint
import android.app.Activity
import android.net.Uri
import android.os.Bundle
import android.os.Message
import android.view.View
import android.view.ViewGroup
import android.webkit.CookieManager
import android.webkit.WebChromeClient
import android.webkit.WebResourceRequest
import android.webkit.WebView
import android.webkit.WebViewClient
import android.widget.FrameLayout
import androidx.appcompat.app.AppCompatActivity

/**
 * Hosts the SF official login page in an application-owned WebView.
 * The activity never reads passwords or injects scripts into the login page.
 */
class SfacgLoginActivity : AppCompatActivity() {
    private lateinit var container: FrameLayout
    private lateinit var webView: WebView
    private val extraWebViews = mutableListOf<WebView>()

    /**
     * Creates the official login flow and allows its OAuth popups.
     *
     * @param savedInstanceState Android state supplied during activity recreation.
     */
    @SuppressLint("SetJavaScriptEnabled")
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        CookieManager.getInstance().setAcceptCookie(true)
        container = FrameLayout(this)
        webView = createWebView()
        container.addView(webView, ViewGroup.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT,
            ViewGroup.LayoutParams.MATCH_PARENT,
        ))
        setContentView(container)
        // 与 Windows 端的 clear_all_browsing_data 对齐：每次新开登录页都从干净会话
        // 开始。否则残留的 .SFCommunity 会让 onPageFinished 在第一屏就判定「已登录」
        // 直接关闭，QQ 的 ptlogin Cookie 也会带出上次账号，用户换不了号。
        // Activity 重建时不清理，避免打断进行中的 OAuth。
        if (savedInstanceState == null) {
            clearWebViewCookies {
                if (!isFinishing && !isDestroyed) webView.loadUrl(OFFICIAL_LOGIN_URL)
            }
        } else {
            webView.loadUrl(OFFICIAL_LOGIN_URL)
        }
    }

    /** Creates a WebView that shares the official login session and OAuth handling. */
    private fun createWebView(): WebView {
        return WebView(this).apply {
            overScrollMode = View.OVER_SCROLL_NEVER
            isVerticalScrollBarEnabled = false
            isHorizontalScrollBarEnabled = false
            settings.javaScriptEnabled = true
            settings.domStorageEnabled = true
            settings.javaScriptCanOpenWindowsAutomatically = true
            settings.setSupportMultipleWindows(true)
            // QQ 互联按 UA 分流：移动 UA 只提供「拉起手机 QQ」的一键登录，而手机 QQ
            // 会把授权结果显式交给系统浏览器，本 WebView 拿不到 Cookie。桌面 UA 下
            // 授权页改为在 iframe 内完成，回调仍落在同一个 WebView。
            settings.userAgentString = DESKTOP_USER_AGENT
            webViewClient = object : WebViewClient() {
                /**
                 * Allows HTTPS plus the legacy HTTP callbacks hosted by SF.
                 *
                 * @param view The WebView requesting navigation.
                 * @param request The requested destination.
                 * @returns True when the navigation is blocked.
                 */
                override fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest): Boolean {
                    val url = request.url
                    return when (url.scheme) {
                        "https" -> false
                        "http" -> url.host?.lowercase() != "passport.sfacg.com"
                        "about" -> url.toString() != "about:blank"
                        else -> true
                    }
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
            webChromeClient = object : WebChromeClient() {
                /**
                 * Displays QQ and WeChat OAuth pages opened with `target="_blank"`.
                 *
                 * @returns True after the popup WebView has been attached.
                 */
                override fun onCreateWindow(
                    view: WebView,
                    isDialog: Boolean,
                    isUserGesture: Boolean,
                    resultMsg: Message,
                ): Boolean {
                    val popup = createWebView()
                    extraWebViews.add(popup)
                    container.addView(popup, ViewGroup.LayoutParams(
                        ViewGroup.LayoutParams.MATCH_PARENT,
                        ViewGroup.LayoutParams.MATCH_PARENT,
                    ))
                    (resultMsg.obj as WebView.WebViewTransport).webView = popup
                    resultMsg.sendToTarget()
                    return true
                }

                /** Removes an OAuth page when the provider closes its popup. */
                override fun onCloseWindow(window: WebView) {
                    if (window === webView) return
                    container.removeView(window)
                    extraWebViews.remove(window)
                    window.destroy()
                }
            }
        }
    }

    /**
     * Finishes only after an SF domain has issued the primary authenticated session cookie.
     *
     * @param url The recently loaded URL used to prevent unrelated pages from completing login.
     */
    private fun finishWhenSessionCookieExists(url: String) {
        if (!isSfacgHost(Uri.parse(url).host)) return
        if (sessionCookieValue() == null) return
        CookieManager.getInstance().flush()
        setResult(Activity.RESULT_OK)
        finish()
    }

    /** 读取有效的 .SFCommunity 值；Cookie 删除留下的空值不算已登录。 */
    private fun sessionCookieValue(): String? {
        val cookie = CookieManager.getInstance().getCookie("https://book.sfacg.com/") ?: return null
        for (part in cookie.split(";")) {
            val pair = part.trim()
            if (!pair.startsWith(".SFCommunity=")) continue
            val value = pair.removePrefix(".SFCommunity=")
            if (value.isNotEmpty()) return value
        }
        return null
    }

    /** Returns whether a host belongs to SF, including its legacy bare domain. */
    private fun isSfacgHost(host: String?): Boolean {
        val normalized = host?.lowercase() ?: return false
        return normalized == "sfacg.com" || normalized.endsWith(".sfacg.com")
    }

    /** Releases the WebView promptly when the login screen is closed. */
    override fun onDestroy() {
        extraWebViews.forEach {
            it.stopLoading()
            it.destroy()
        }
        extraWebViews.clear()
        if (::webView.isInitialized) {
            webView.stopLoading()
            webView.destroy()
        }
        super.onDestroy()
    }

    private companion object {
        /** The SF-hosted entrypoint where users complete their own authentication. */
        const val OFFICIAL_LOGIN_URL = "https://passport.sfacg.com/"
        /** 与 Rust 侧 `SF_WEB_USER_AGENT` 一致的桌面浏览器 UA，只作用于登录 WebView。 */
        const val DESKTOP_USER_AGENT =
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36"
    }
}
