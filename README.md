# FreeSheet

A free, fully offline study app for South African learners following the **CAPS** curriculum
(English medium), Grades 8 to 12. It works on Windows, macOS and Linux.

* **Study**: notes for every topic, organised by grade, subject and term, with practice questions on the
  notes you have just read. Multiple-choice, true/false, matching, one-word and calculation answers are
  marked instantly. For written answers you compare your work with the memo and award your own marks.
* **Exams**: full exam papers in CAPS format with an optional countdown timer, automatic saving (you can
  leave and resume), self-marking with the memo, results with CAPS achievement levels, and a review of
  every answer.
* **Progress**: topics practised, scores per subject, exam history, plus export/import/reset of your data.
* **Hints** on practice questions (never in exams), getting stronger each time you ask: the sentence in
  the notes that explains it (with the answer blanked out) and a link to that section, ruling out wrong
  options, the answer's letter pattern, the first step of a calculation, one matching pair, key words
  for written answers. Typed answers forgive small spelling mistakes, except in language subjects.
* **Study buddy**: an animated character you design and name when you first open the app (or after
  updating). It cheers right answers, offers to show where a missed answer is explained, suggests what to
  study next from your progress, reacts to streaks and results, and has a tap-for-help menu. It steps out
  during exams. Change it, switch it off, or make it speak only when tapped, in Settings.
* **Settings** (cog, top right): text size, animations, sound effects and the study buddy.

There are no accounts and nothing is sent over the internet. Progress is saved to a single file,
`progress.json`, in your user-data folder:

| OS      | Location |
|---------|----------|
| Windows | `%APPDATA%\FreeSheet\progress.json` |
| macOS   | `~/Library/Application Support/FreeSheet/progress.json` |
| Linux   | `~/.config/FreeSheet/progress.json` |

## Subjects

* **Grades 8–9:** English Home Language, Mathematics, Natural Sciences, Social Sciences (History and
  Geography as separate subjects), Economic and Management Sciences, Technology, Life Orientation.
* **Grades 10–12:** English Home Language, Mathematics, Mathematical Literacy, Physical Sciences,
  Life Sciences, Geography, History, Accounting, Business Studies, Economics, Life Orientation.

## Running from source

You need [Node.js](https://nodejs.org) 18 or newer.

```sh
npm install
npm start
```

## Building installers

```sh
npm run build          # everything this computer can build
npm run build:win      # Windows: FreeSheet-Setup-<version>.exe and FreeSheet-<version>.msi
npm run build:mac      # macOS: .dmg and .zip for Intel and Apple Silicon (needs a Mac)
npm run build:linux    # Linux: .AppImage, .deb and .tar.gz
```

Everything is written to the `build/` folder. The script checks all content first (skip with
`--no-validate`), then prints a summary of what was built and what was skipped.

What each computer can build:

| Building on | Windows                | macOS | Linux |
|-------------|------------------------|-------|-------|
| Windows     | .exe, .msi             | no    | .tar.gz; .AppImage if Developer Mode is on; no .deb |
| macOS       | .exe (no .msi)         | yes   | yes   |
| Linux       | .exe with Wine (no .msi) | no  | yes   |

The script runs the same way on every OS (`npm run build`). Platforms or formats a computer can't produce are
skipped with the reason shown, and don't count as a failure. The Linux `.deb` needs a project homepage: add
`"homepage"` to `package.json`, or the script takes it from the git remote (or from GitHub Actions).

Builds are unsigned unless signing certificates are provided through electron-builder's usual environment
variables (`CSC_LINK`, `CSC_KEY_PASSWORD`, and for macOS notarisation `APPLE_ID` and related). Unsigned macOS
builds are ad-hoc signed so they still open on Apple Silicon Macs.

To build **all** of them at once, push the project to GitHub and run the **Build installers** workflow
(`.github/workflows/build.yml`) from the Actions tab. It builds on real Windows, macOS and Linux machines and
attaches the installers to the run. The apps are not code-signed, so Windows SmartScreen and macOS Gatekeeper
will warn on first launch (on macOS, right-click the app and choose **Open**).

## Content

All notes, questions and exams are JSON files in `content/grade-N/<subject>/`. The format is documented in
[docs/CONTENT_FORMAT.md](docs/CONTENT_FORMAT.md). Run `npm test` to validate every file (structure, answer keys,
marks totals, and that all maths renders) and run the marking unit tests.

The content is original and written to follow the CAPS documents and Annual Teaching Plans. It is not
affiliated with the Department of Basic Education or any commercial service.

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
`audio/` folder is not committed to git; the build copies it into the installers. Builds made without it
(for example on GitHub Actions) simply hide the Listen button.

## Development

* `main.js`: Electron main process (window, content loading, saving progress)
* `preload.js`: the small API exposed to the page
* `src/`: the interface (plain HTML/CSS/JS, no build step); maths is rendered with KaTeX
* `src/js/hints.js`: hints for practice questions; `src/js/buddy.js`: the study buddy (art, animation,
  speech bubble, setup); what the buddy says is decided in `src/js/app.js` ("Study buddy")
* `scripts/validate-content.js`: content validator; `scripts/test-marking.js`: marking and hint tests
  (including a check that no hint in the content gives its answer away)
* `scripts/screenshot.js`: clicks through the app and saves screenshots
  (`npx electron scripts/screenshot.js <outDir> [grade] [subjectId]`)
