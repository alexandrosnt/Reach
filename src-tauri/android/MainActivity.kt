package com.reach.desktop

import android.os.Bundle
import android.view.View
import android.webkit.JavascriptInterface
import android.webkit.WebView
import androidx.activity.enableEdgeToEdge
import androidx.core.graphics.Insets
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat

// Copied over the activity `tauri android init` generates (see the Android
// jobs in .github/workflows), which is the same apart from the keyboard, the
// screen edges and the text size below.
class MainActivity : TauriActivity() {
  private var webView: WebView? = null

  /** The system bars and camera cutout, as CSS "top right bottom left". */
  @Volatile private var edges = "0px 0px 0px 0px"

  override fun onWebViewCreate(webView: WebView) {
    super.onWebViewCreate(webView)
    // The WebView scales all text by the phone's font size, up to twice as
    // large, while boxes keep their size, so Reach's rows and menus
    // overflowed. A larger font size is followed up to the largest one every
    // screen is tested at.
    webView.settings.textZoom = minOf(webView.settings.textZoom, MAX_TEXT_ZOOM)
    // The page asks for the screen edges when it starts (see edges.ts); this
    // gives it numbers only.
    webView.addJavascriptInterface(EdgesBridge(), "ReachEdges")
    this.webView = webView
  }

  private inner class EdgesBridge {
    @JavascriptInterface fun get(): String = edges
  }

  companion object {
    const val MAX_TEXT_ZOOM = 130
  }

  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)

    val content = findViewById<View>(android.R.id.content)
    ViewCompat.setOnApplyWindowInsetsListener(content) { view, insets ->
      // Drawn edge to edge, the window is no longer made smaller for the
      // keyboard (Android 15 ignores adjustResize), so the page went on under
      // it and the keyboard hid the terminal's extra-keys bar (#127). The page
      // is lifted by the part of the keyboard above the navigation bar, which
      // the page keeps clear of itself (below).
      val keyboard = insets.getInsets(WindowInsetsCompat.Type.ime()).bottom
      val bars = insets.getInsets(WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout())
      view.setPadding(0, 0, 0, maxOf(keyboard - bars.bottom, 0))

      // The WebView tells the page about the camera cutout only, not the
      // status and navigation bars, so on its side the page went under the
      // clock and the gesture bar. The page is told where they all are, and
      // again whenever they move (rotation, bars shown or hidden).
      val d = resources.displayMetrics.density
      val css = "${bars.top / d}px ${bars.right / d}px ${bars.bottom / d}px ${bars.left / d}px"
      if (css != edges) {
        edges = css
        webView?.post { webView?.evaluateJavascript("window.__reachEdges?.('$css')", null) }
      }

      // The keyboard's inset is used up, so a WebView that would make room
      // for the keyboard on its own does not do so a second time.
      WindowInsetsCompat.Builder(insets).setInsets(WindowInsetsCompat.Type.ime(), Insets.NONE).build()
    }
  }
}
