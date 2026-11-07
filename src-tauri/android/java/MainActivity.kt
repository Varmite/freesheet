package za.freesheet.app

// FreeSheet's Android activity (copied over Tauri's generated one by scripts/build-android.js).
// Android 15+ draws apps edge to edge, under the status and navigation bars. Rather than rely
// on the WebView reporting safe-area insets (older WebViews do not), the page is padded clear
// of the system bars, display cutout and on-screen keyboard. The padding shows the app's
// background colour (light or dark, as the page does).

import android.content.res.Configuration
import android.os.Bundle
import android.view.View
import androidx.activity.enableEdgeToEdge
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat

class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
    val root = findViewById<View>(android.R.id.content)
    val night = (resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK) == Configuration.UI_MODE_NIGHT_YES
    root.setBackgroundColor(if (night) 0xFF161616.toInt() else 0xFFFFFFFF.toInt())
    ViewCompat.setOnApplyWindowInsetsListener(root) { v, insets ->
      val bars = insets.getInsets(WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout())
      val ime = insets.getInsets(WindowInsetsCompat.Type.ime())
      v.setPadding(bars.left, bars.top, bars.right, maxOf(bars.bottom, ime.bottom))
      WindowInsetsCompat.CONSUMED
    }
  }
}
