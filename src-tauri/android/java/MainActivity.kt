package za.freesheet.app

// FreeSheet's Android activity (copied over Tauri's generated one by scripts/build-android.js).
// Android 15+ draws apps edge to edge, under the status and navigation bars. Rather than rely
// on the WebView reporting safe-area insets (older WebViews do not), the page is padded clear
// of the system bars, display cutout and on-screen keyboard. The padding shows the app's
// background colour (light or dark, as the page does).
//
// Phones short of memory: Android may kill the WebView's separate renderer process (or it can
// crash). Left alone that takes the whole app down, or leaves a dead white page. Here the app
// starts again cleanly instead; progress is already saved by then (the page saves as it goes).

import android.content.Context
import android.content.Intent
import android.content.res.Configuration
import android.graphics.Bitmap
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.os.SystemClock
import android.provider.Settings
import android.view.View
import android.webkit.JavascriptInterface
import android.webkit.RenderProcessGoneDetail
import android.webkit.WebResourceError
import android.webkit.WebResourceRequest
import android.webkit.WebResourceResponse
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.activity.enableEdgeToEdge
import androidx.annotation.RequiresApi
import androidx.core.view.ViewCompat
import androidx.core.content.FileProvider
import androidx.core.view.WindowInsetsCompat
import androidx.lifecycle.Lifecycle
import androidx.webkit.WebViewCompat
import java.io.File

class MainActivity : TauriActivity() {
  override fun onWebViewCreate(webView: WebView) {
    webView.addJavascriptInterface(UpdateBridge(), "FreeSheetAndroid")
    // Tauri sets its own WebViewClient straight after this call; once it has, wrap it.
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
      Handler(Looper.getMainLooper()).post { guardRenderer(webView) }
    }
  }

  @RequiresApi(Build.VERSION_CODES.O)
  private fun guardRenderer(webView: WebView) {
    val inner = webView.webViewClient
    if (inner !is RendererGuard) webView.webViewClient = RendererGuard(inner)
  }

  // Passes everything Tauri's client handles straight through, and adds the one thing it
  // lacks: surviving the loss of the renderer process (Android 8+ reports it here).
  @RequiresApi(Build.VERSION_CODES.O)
  inner class RendererGuard(private val inner: WebViewClient) : WebViewClient() {
    override fun shouldInterceptRequest(view: WebView, request: WebResourceRequest): WebResourceResponse? =
      inner.shouldInterceptRequest(view, request)
    override fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest): Boolean =
      inner.shouldOverrideUrlLoading(view, request)
    override fun onPageStarted(view: WebView, url: String, favicon: Bitmap?) = inner.onPageStarted(view, url, favicon)
    override fun onPageFinished(view: WebView, url: String) = inner.onPageFinished(view, url)
    override fun onReceivedError(view: WebView, request: WebResourceRequest, error: WebResourceError) =
      inner.onReceivedError(view, request, error)
    override fun onRenderProcessGone(view: WebView, detail: RenderProcessGoneDetail): Boolean {
      restartAfterRendererLoss()
      return true
    }
  }

  // The page and the Rust side cannot carry on without the renderer, so the whole app starts
  // afresh: straight away if the learner is looking at it (RestartActivity, in a process of
  // its own, ends this one and opens the app again), otherwise the next time they open it.
  // Not twice within 15 seconds, so a WebView that dies on every start cannot loop.
  private fun restartAfterRendererLoss() {
    try {
      val prefs = getSharedPreferences("freesheet", Context.MODE_PRIVATE)
      val now = SystemClock.elapsedRealtime()
      val last = prefs.getLong("rendererRestart", 0L)
      val looping = last in 1..now && now - last < 15_000
      val visible = lifecycle.currentState.isAtLeast(Lifecycle.State.STARTED)
      if (visible && !looping) {
        prefs.edit().putLong("rendererRestart", now).commit()
        startActivity(Intent(this, RestartActivity::class.java)
          .putExtra(RestartActivity.EXTRA_PID, android.os.Process.myPid())
          .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
        return
      } else if (visible) {
        finishAffinity()
      }
    } catch (e: Exception) {
      // Fall through: closing is still better than a dead page.
    }
    Runtime.getRuntime().exit(0)
  }

  // Hands an update that the app has already downloaded and checked to Android's installer.
  // Android always asks the learner to confirm. Takes only a version, so the page can never
  // point it at another file.
  inner class UpdateBridge {
    @JavascriptInterface
    fun installUpdate(version: String): String {
      if (!version.matches(Regex("[0-9]+[.][0-9]+[.][0-9]+"))) return "failed"
      val apk = File(File(cacheDir, "updates"), "FreeSheet-$version-android.apk")
      if (!apk.isFile) return "failed"
      return try {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O && !packageManager.canRequestPackageInstalls()) {
          startActivity(Intent(Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES, Uri.parse("package:$packageName")))
          "permission"
        } else {
          val uri = FileProvider.getUriForFile(this@MainActivity, "$packageName.fileprovider", apk)
          startActivity(Intent(Intent.ACTION_VIEW).setDataAndType(uri, "application/vnd.android.package-archive").addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_ACTIVITY_NEW_TASK))
          "started"
        }
      } catch (e: Exception) {
        "failed"
      }
    }

    // Opens the Play Store page of the app that provides this phone's WebView (Android System
    // WebView, or Chrome on Android 7 to 9), for the "update needed" notice in js/compat.js.
    @JavascriptInterface
    fun openWebViewUpdate(): Boolean {
      val pkg = try {
        WebViewCompat.getCurrentWebViewPackage(this@MainActivity)?.packageName
      } catch (e: Exception) {
        null
      } ?: "com.google.android.webview"
      for (url in listOf("market://details?id=$pkg", "https://play.google.com/store/apps/details?id=$pkg")) {
        try {
          startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(url)).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
          return true
        } catch (e: Exception) {
          // No Play Store: try the web page.
        }
      }
      return false
    }
  }

  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
    val root = findViewById<View>(android.R.id.content)
    val night = (resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK) == Configuration.UI_MODE_NIGHT_YES
    root.setBackgroundColor(if (night) 0xFF1A1B1E.toInt() else 0xFFFAF8F4.toInt())
    ViewCompat.setOnApplyWindowInsetsListener(root) { v, insets ->
      val bars = insets.getInsets(WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout())
      val ime = insets.getInsets(WindowInsetsCompat.Type.ime())
      v.setPadding(bars.left, bars.top, bars.right, maxOf(bars.bottom, ime.bottom))
      WindowInsetsCompat.CONSUMED
    }
  }
}
