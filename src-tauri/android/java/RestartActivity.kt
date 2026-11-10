package za.freesheet.app

// Starts FreeSheet again after Android killed the WebView's renderer (see MainActivity.kt).
// It runs in a process of its own (android:process=":restart", added to the manifest by
// scripts/build-android.js), so it can end the old app process completely and only then
// open the app afresh. Doing both from the dying process itself sometimes lost the race
// and left the learner on the home screen.

import android.app.Activity
import android.content.Intent
import android.os.Bundle
import android.os.Process

class RestartActivity : Activity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    super.onCreate(savedInstanceState)
    val old = intent.getIntExtra(EXTRA_PID, -1)
    if (old > 0 && old != Process.myPid()) Process.killProcess(old)
    packageManager.getLaunchIntentForPackage(packageName)?.let {
      startActivity(it.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TASK))
    }
    finish()
    Runtime.getRuntime().exit(0)
  }

  companion object {
    const val EXTRA_PID = "za.freesheet.app.oldPid"
  }
}
