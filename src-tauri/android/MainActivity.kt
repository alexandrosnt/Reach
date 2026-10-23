package com.reach.desktop

import android.os.Bundle
import android.view.View
import androidx.activity.enableEdgeToEdge
import androidx.core.graphics.Insets
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat

// Copied over the activity `tauri android init` generates (see the Android
// jobs in .github/workflows), which is the same apart from the keyboard below.
class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)

    // Drawn edge to edge, the window is no longer made smaller for the
    // keyboard (Android 15 ignores adjustResize), so the page went on under
    // it and the keyboard hid the terminal's extra-keys bar (#127). The page
    // is lifted by the part of the keyboard above the navigation bar; it
    // keeps clear of that bar itself, with env(safe-area-inset-bottom). The
    // keyboard's inset is then used up, so a WebView that would make room
    // for the keyboard on its own does not do so a second time.
    val content = findViewById<View>(android.R.id.content)
    ViewCompat.setOnApplyWindowInsetsListener(content) { view, insets ->
      val keyboard = insets.getInsets(WindowInsetsCompat.Type.ime()).bottom
      val bars = insets.getInsets(WindowInsetsCompat.Type.systemBars()).bottom
      view.setPadding(0, 0, 0, maxOf(keyboard - bars, 0))
      WindowInsetsCompat.Builder(insets).setInsets(WindowInsetsCompat.Type.ime(), Insets.NONE).build()
    }
  }
}
