package com.sansan.sf_novel_flow

import android.os.Bundle
import androidx.activity.enableEdgeToEdge

class MainActivity : TauriActivity() {
  /**
   * Enables edge-to-edge rendering before Tauri initializes the main activity.
   *
   * @param savedInstanceState Android state restored after process recreation.
   */
  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
  }
}
