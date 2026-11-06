# FreeSheet Android Edition

FreeSheet for Android phones (Android 7.0 and newer). It is the same app as the desktop edition, with
the same notes, practice questions, exams, Afrikaans courses, read-aloud audio, hints, study buddy,
streaks and settings, laid out for a phone screen. It works fully offline.

## What's different on a phone

* **Tabs at the bottom.** Study, Exams and Progress sit in a bar at the bottom of the screen, where your
  thumb reaches. The streak, grade and settings stay in the top bar.
* **Dialogs rise from the bottom** as sheets with full-width buttons.
* **Lessons fill the screen.** The tab bar and buddy step aside during a language-course lesson.
* **Back button and gesture** step back through the app: close a dialog, leave a lesson or Settings,
  go up one level, then return to Study. From the Study home it sends the app to the background.
* **Saving:** progress is saved on the phone (inside the app). It is also saved whenever you leave the
  app, and an exam in progress is kept so you can resume it.
* **Export** saves a copy to `Documents/FreeSheet/` and opens the share sheet (Drive, email, WhatsApp,
  and so on). **Import** opens the phone's file picker. These files work with the desktop edition too,
  so you can move your progress between a computer and a phone.

## How it is built

The Android Edition does not keep its own copy of the app. `scripts/prepare-web.js` builds `www/` from
the desktop edition every time:

| From | To | Notes |
|---|---|---|
| `../src` | `www/` | the app, plus `web/android-api.js` and `web/mobile.css` |
| `../content` | `www/content/` | all notes, questions, exams and courses |
| `../audio` | `www/audio/` | read-aloud audio |
| KaTeX | `www/katex/` | maths typesetting |

`web/android-api.js` stands in for the desktop's `preload.js`/`main.js`: it gives the app the same
`window.api`, reading content with `fetch()` and saving progress on the phone. The lists the desktop
makes by reading folders (catalogue, word list, which topics have audio) are made at build time into
`www/data/`. [Capacitor](https://capacitorjs.com) wraps `www/` in an Android app (`android/`).

So any change to the desktop app's features or content is in the next Android build with no extra work.
Phone-only layout belongs in `web/mobile.css`.

## Building

**On GitHub (no setup needed):** run the **Build Android Edition** workflow:

```sh
gh workflow run android.yml --ref master
gh run download <run-id> -n FreeSheet-android -D build/android
```

**On a computer with the Android SDK and JDK 21:**

```sh
cd "Android Edition"
npm ci
npm run build        # -> ../build/android/FreeSheet-<version>-android.apk
```

The version number is the desktop edition's (`../package.json`), so both editions always match.

**Preview the phone layout** without a phone (uses the desktop edition's Electron, from the repo root):

```sh
node "Android Edition/scripts/prepare-web.js"
npx electron "Android Edition/scripts/phone-preview.js" <folder-for-screenshots>
```

**Icons and splash screens** are made from `../build-resources/icon.png` by
`python scripts/make-icons.py`.

## Signing

Android only installs an update over an existing install if both are signed with the same key. The
release key is `keystore/freesheet-release.jks`, with its passwords in `keystore/keystore.properties`.
It is not in git. **Keep a backup of the `keystore/` folder.** If it is lost, people have to uninstall the
app (and lose progress they have not exported) before they can install a new version.

GitHub Actions signs with the same key, stored as repository secrets: `ANDROID_KEYSTORE_B64` (the
.jks file, base64), `ANDROID_KEYSTORE_PASSWORD`, `ANDROID_KEY_ALIAS` and `ANDROID_KEY_PASSWORD`. Without
them it signs with a debug key and shows a warning.

## Installing on a phone

The APK is not on the Play Store. Download `FreeSheet-<version>-android.apk` on the phone (from the
GitHub release), open it, and allow your browser or file manager to **install unknown apps** when Android
asks. Then tap **Install**.
