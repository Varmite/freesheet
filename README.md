# FreeSheet

A free, fully offline study app for South African learners following the **CAPS** curriculum
(English medium), Grades 8 to 12. It works on Windows, macOS and Linux, and on Android phones
(the **Android Edition**).

Since version 1.1 FreeSheet is a [Tauri](https://tauri.app) app written in Rust (`src-tauri/`), not an
Electron one. It draws its screens with the web view already built into the system (WebView2 on
Windows, WebKit on macOS and Linux, Android's WebView on phones) instead of bundling its own copy of
Chromium. That makes the program about 10 MB instead of about 360 MB, and uses less memory. On Windows
it takes roughly 105 MB while open (Electron: about 135 MB) and about 60 MB when minimised. The same
Rust code is used on Android.

* **Study**: notes for every topic, organised by grade, subject and term, with practice questions on the
  notes you have just read. Multiple-choice, true/false, matching, one-word and calculation answers are
  marked instantly. For written answers you compare your work with the memo and award your own marks.
* **Exams**: full exam papers in CAPS format with an optional countdown timer, automatic saving (you can
  leave and resume), self-marking with the memo, results with CAPS achievement levels, and a review of
  every answer.
* **AI marking** (optional, computers only, off until you turn it on in Settings): an AI model that runs
  on your own computer marks written answers against the memo, point by point, and you can change any
  mark. It needs a one-time 1.1 GB download and nearly 5 GB of memory while marking, and your answers
  never leave the computer. Essays marked with a rubric are still marked by you.
* **Progress**: topics practised, scores per subject, exam history, plus export/import/reset of your data.
* **Hints** on practice questions (never in exams), getting stronger each time you ask: the sentence in
  the notes that explains it (with the answer blanked out) and a link to that section, ruling out wrong
  options, the answer's letter pattern, the first step of a calculation, one matching pair, key words
  for written answers. Typed answers forgive small spelling mistakes, except in language subjects.
* **Study buddy**: an animated character you design and name when you first open the app (or after
  updating). It cheers right answers, offers to show where a missed answer is explained, suggests what to
  study next from your progress, reacts to streaks and results, and has a tap-for-help menu. It steps out
  during exams. Change it, switch it off, or make it speak only when tapped, in Settings.
* **Settings** (cog, top right, or Ctrl+,): colour themes (Automatic, Light, Dark, Sepia, Midnight, Forest,
  Black, High contrast) and accent colours, text size, text rendering, line spacing, reading width, the tab
  to open on, animations, sound effects, the study buddy and updates.
* **Updates**: each time it opens, FreeSheet quietly checks GitHub for a newer version and shows a small
  card if there is one (never during an exam). It downloads nothing by itself. Turn it off in Settings.
* **Smooth text** (default): text is drawn the way macOS draws it: greyscale anti-aliasing instead of
  ClearType's colour fringes and the Inter typeface (SIL Open Font License, `src/fonts`), tightened only
  at heading sizes, with a slightly heavier weight for dark-on-light text on Windows. "System" switches back to Windows' own rendering.

There are no accounts and none of your data is sent over the internet: the update check is the app's only
connection, a plain request for the public list of releases, and without internet everything still works. Progress is saved to a single file,
`progress.json`, in your user-data folder:

| OS      | Location |
|---------|----------|
| Windows | `%APPDATA%\FreeSheet\progress.json` |
| macOS   | `~/Library/Application Support/FreeSheet/progress.json` |
| Linux   | `~/.config/FreeSheet/progress.json` |
| Android | inside the app (export it from Settings to keep a copy) |

These are the same places version 1.0 used, so updating keeps your progress. On Windows the 1.1
installer also removes the old 1.0 program.

## Subjects

* **Grades 8–9:** English Home Language, Mathematics, Natural Sciences, Social Sciences (History and
  Geography as separate subjects), Economic and Management Sciences, Technology, Life Orientation.
* **Grades 10–12:** English Home Language, Mathematics, Mathematical Literacy, Physical Sciences,
  Life Sciences, Geography, History, Accounting, Business Studies, Economics, Life Orientation.

## Running from source

You need [Node.js](https://nodejs.org) 18 or newer and [Rust](https://rustup.rs). On Linux also install
the WebKitGTK packages (Ubuntu/Debian: `sudo apt install libwebkit2gtk-4.1-dev librsvg2-dev
libxdo-dev libssl-dev build-essential`).

```sh
npm install
npm start          # tauri dev: builds the Rust side and opens the app
```

## Building installers

```sh
npm run build          # installers for this computer's system
npm run build:win      # Windows: FreeSheet-Setup-<version>.exe and FreeSheet-<version>.msi
npm run build:mac      # macOS: .dmg and .zip for Intel and Apple Silicon (needs a Mac)
npm run build:linux    # Linux: .AppImage and .deb (needs Linux)
npm run build:android  # Android: .apk (needs the Android SDK and NDK)
```

Everything is written to the `build/` folder. The script asks for the new version number, checks all
content (skip with `--no-validate`), then prints a summary. Tauri builds each system's app on that system,
so to build **all** of them use GitHub Actions:

* **Build installers** (`.github/workflows/build.yml`) builds Windows, both Macs and Linux on real
  machines: `gh workflow run build.yml --ref master` (add `-f platforms=mac-linux` if Windows was already
  built on a PC). Download the results with `gh run download <run-id> -D build`.
* **Build Android Edition** (`.github/workflows/android.yml`) builds the APK:
  `gh workflow run android.yml --ref master`.

The apps are not code-signed, so Windows SmartScreen and macOS Gatekeeper will warn on first launch (on
macOS, right-click the app and choose **Open**). Mac builds are ad-hoc signed so they open on Apple Silicon.
Read-aloud uses Ogg Opus audio. macOS plays it from macOS 15.4 onwards (older Macs show the notes without
audio). The Linux `.deb` pulls in GStreamer's "good" plugins for playback.

### Android Edition

The phone app is built from the same Rust code and screens. On a phone the layout changes
(`src/mobile.css`, loaded only on Android): the tabs sit at the bottom, dialogs rise from the bottom
as sheets, and the back button or gesture steps back through the app. Progress is saved inside the app
whenever you leave it. **Export** saves the progress file wherever you choose (Drive, Downloads...) and
**Import** opens the file picker. The files work on the desktop too.

`scripts/build-android.js` makes the Android project (`src-tauri/gen/android`, not committed), copies in
the launcher icons (`src-tauri/android/res`) and the read-aloud audio, and signs the APK. The release
signing key is in `src-tauri/android/keystore/` (never committed; **keep a backup**, because without it
phones cannot install updates). `bash scripts/set-github-secrets.sh` copies it into the GitHub secrets the
workflow uses. Versions up to 1.0.1 were a Capacitor app. 1.1 uses the same package name and key, so it
installs over them and keeps their progress.

## Content

All notes, questions and exams are JSON files in `content/grade-N/<subject>/`. The format is documented in
[docs/CONTENT_FORMAT.md](docs/CONTENT_FORMAT.md). Run `npm test` to validate every file (structure, answer keys,
marks totals, and that all maths renders) and run the marking unit tests.

The content is original and written to follow the CAPS documents and Annual Teaching Plans. It is not
affiliated with the Department of Basic Education or any commercial service.

### Language courses (Afrikaans)

Afrikaans appears as a subject in every grade, with a course at that grade's CAPS First Additional
Language level (`content/courses/afrikaans-gr8` … `afrikaans-gr12`): Grade 8 word order, questions,
negatives, past tense, plurals and diminutives; Grade 9 future, modal verbs, om ... te, conjunctions and
pronouns; Grade 10 the passive and indirect speech; Grade 11 idioms, proverbs, figurative language and
formal writing; Grade 12 literature terms, persuasive language and complex sentences.

It is taught as a gamified language course: units of short lessons that unlock in order, new words
introduced one at a time, then exercises generated from each lesson's words, sentences and grammar
drills (pick the meaning, match pairs, translate with word tiles both ways, type a translation, fill in
the blank, and rewrite a sentence as told: past tense, passive, indirect speech, join with "omdat"...).
Mistakes come back at the end of the lesson; answers forgive case, punctuation, contractions,
synonyms, missing accents and small typos (with a note), but not a typo that makes a different real
word, nor a missing or extra word. Lessons earn XP and count towards the daily streak, and your study
buddy is the character who speaks the sentences and reacts.

Each grade's `course.json` has units → lessons → `words`, `sentences` (with every accepted
translation) and `drills` (`task`, `from`, accepted `to`); the engine is `src/js/course.js` and the
screens `src/js/course-ui.js`. `node scripts/test-course.js` (part of `npm test`) generates every lesson
several ways and checks that every exercise can be solved, every accepted answer is accepted, and an
unchanged sentence never passes a drill.

Listening: there is no good offline Afrikaans text-to-speech voice (Meta's MMS and Piper have none), so
the course has no audio yet. `node scripts/tts/course-extract.js` writes `tts/course-phrases.csv`: every
word and sentence with the file name the app expects. Recordings saved as
`audio/courses/<course>/<id>.ogg` are picked up automatically: speaker buttons appear and "Tap what you
hear" exercises switch on.

## Read-aloud audio

Notes are shown one section at a time (each heading starts a new section). Every section can be read aloud
by a pre-recorded voice that ships inside the installers, so it works fully offline. The audio is generated
on your own computer with [Kokoro-82M](https://huggingface.co/hexgrad/Kokoro-82M) (Apache-2.0), using an
NVIDIA GPU if one is available.

One-time setup (Python 3.10–3.12 and ffmpeg on the PATH):

```
python -m venv tts\.venv
tts\.venv\Scripts\python -m pip install torch --index-url https://download.pytorch.org/whl/cu128
tts\.venv\Scripts\python -m pip install "kokoro>=0.9.4" soundfile
```

Then run `npm run tts` before building. It converts each section to speech text with `src/js/sections.js`
(maths is read in words, e.g. `x^2` becomes "x squared"), generates Ogg Opus files in `audio/`, skips sections
that already have audio, and deletes audio for sections that no longer exist. Each file is named by a hash of
its text, so editing a section automatically gives it new audio the next time you run `npm run tts`. The
`audio/` folder is committed, and the build copies it into the installers (and the APK). Sections without
audio simply hide their Listen button.

## Development

* `src-tauri/`: the Rust app (Tauri 2)
  * `src/lib.rs`: the window, the commands the page calls, and saving on close
  * `src/content.rs`: notes, questions, exams and courses. The `content/` folder is built into the
    program, and the catalogue and spell-check word list are made from it
  * `src/progress.rs`: `progress.json` (written safely through a temporary file, with a `.bak` copy)
  * `src/audio.rs`: read-aloud audio (from the install folder, or from the APK on Android)
  * `tauri.conf.json`: app and installer settings; `cargo test` runs the Rust tests
* `src/`: the interface (plain HTML/CSS/JS, no build step); maths is rendered with KaTeX (`src/vendor`)
  * `src/js/api.js`: the bridge to the Rust side (`window.api`), plus the Android back button
* `src/js/hints.js`: hints for practice questions; `src/js/buddy.js`: the study buddy (art, animation,
  speech bubble, setup); what the buddy says is decided in `src/js/app.js` ("Study buddy")
* `scripts/validate-content.js`: content validator; `scripts/test-marking.js`: marking and hint tests
  (including a check that no hint in the content gives its answer away)
