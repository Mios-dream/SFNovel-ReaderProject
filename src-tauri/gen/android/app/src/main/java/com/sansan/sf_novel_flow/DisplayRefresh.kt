package com.sansan.sf_novel_flow

import android.app.Activity
import android.content.Context
import android.os.Build

/**
 * Keeps the window in sync with the user's refresh-rate preference.
 *
 * Android drives ordinary windows at 60Hz unless they ask otherwise, so on a 120Hz
 * panel every frame is presented twice and scrolling looks stepped even when the
 * page keeps its frame budget. `preferredRefreshRate` is deprecated but stays the
 * only public lever for non-SurfaceView windows; a ROM that ignores it keeps its
 * own default.
 *
 * The preference lives in two places on purpose: the settings file owns the single
 * source of truth, and the copy here lets the window be created at the right rate
 * before Rust has initialized. Rust rewrites it whenever the setting changes.
 */
object DisplayRefresh {
    private const val PREFS = "sf_display"
    private const val KEY_PEAK = "peak_refresh_rate"

    /** Whether the user wants the display's highest refresh rate. Defaults to on. */
    fun isPeakEnabled(context: Context): Boolean =
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE).getBoolean(KEY_PEAK, true)

    /** Persists the preference without touching the running window. */
    fun setPeakEnabled(context: Context, enabled: Boolean) {
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
            .edit().putBoolean(KEY_PEAK, enabled).apply()
    }

    /** Applies the stored preference to the window; call on the UI thread. */
    fun apply(activity: Activity) {
        val target = if (isPeakEnabled(activity)) peakRate(activity) else 60f
        activity.window.attributes = activity.window.attributes.apply {
            @Suppress("DEPRECATION")
            preferredRefreshRate = target
        }
    }

    /** Highest refresh rate the panel reports; 0 means "no preference". */
    private fun peakRate(activity: Activity): Float {
        val current = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            activity.display
        } else {
            @Suppress("DEPRECATION")
            activity.windowManager.defaultDisplay
        } ?: return 0f
        @Suppress("DEPRECATION")
        return current.supportedModes.maxByOrNull { it.refreshRate }?.refreshRate ?: 0f
    }
}
