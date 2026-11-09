package za.freesheet.app

// FreeSheet's Android activity (copied over Tauri's generated one by scripts/build-android.js).
// Android 15+ draws apps edge to edge, under the status and navigation bars. Rather than rely
// on the WebView reporting safe-area insets (older WebViews do not), the page is padded clear
// of the system bars, display cutout and on-screen keyboard. The padding shows the app's
// background colour (light or dark, as the page does).

import android.content.Intent
import android.content.res.Configuration
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.provider.Settings
import android.view.View
import android.webkit.JavascriptInterface
import android.webkit.WebView
import androidx.activity.enableEdgeToEdge
import androidx.core.view.ViewCompat
import androidx.core.content.FileProvider
import androidx.core.view.WindowInsetsCompat
import java.io.File

class MainActivity : TauriActivity() {
  override fun onWebViewCreate(webView: WebView) {
    webView.addJavascriptInterface(UpdateBridge(), "FreeSheetAndroid")
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
