# FreeSheet

A free, fully offline study app for South African learners following the **CAPS** curriculum
(English medium), Grades 8 to 12. It works on Windows, macOS and Linux, and on Android phones
(the **Android Edition**, in [`Android Edition/`](Android%20Edition/README.md)).

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
* **Settings** (cog, top right): text size, text rendering, animations, sound effects and the study buddy.
* **Smooth text** (default): text is drawn the way macOS draws it: greyscale anti-aliasing instead of
  ClearType's colour fringes and the Inter typeface (SIL Open Font License, `src/fonts`), tightened only
  at heading sizes, with a slightly heavier weight for dark-on-light text on Windows. "System" switches back to Windows' own rendering.

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
attaches the installers to the run. The Intel and Apple Silicon Mac apps build on separate machines at
the same time. If you have already built Windows on a PC, choose **platforms: mac-linux** to skip it:
`gh workflow run build.yml -f platforms=mac-linux`. The apps are not code-signed, so Windows SmartScreen and macOS Gatekeeper
will warn on first launch (on macOS, right-click the app and choose **Open**).

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
