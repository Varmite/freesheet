# FreeSheet content format (schemaVersion 1)

All study content lives in `content/` as plain JSON (UTF-8, no BOM). The app discovers
content by scanning folders, so adding a subject never requires editing a shared index.

```
content/
  grade-10/
    mathematics/
      subject.json          subject metadata + ordered topic/exam lists
      topics/<topicId>.json one file per topic: notes + practice questions
      exams/<examId>.json   one file per full exam paper
```

Validate everything with `npm run validate` (or `node scripts/validate-content.js content/grade-10/mathematics`
for one subject). The validator also test-renders every `$...$` maths segment with KaTeX.

## IDs

IDs (`subject id`, `topicId`, `examId`, question `id`) are lowercase kebab-case: `^[a-z0-9]+(-[a-z0-9]+)*$`.
File names must equal the id: `topics/functions-and-graphs.json` has `"id": "functions-and-graphs"`.

## subject.json

```json
{
  "schemaVersion": 1,
  "grade": 10,
  "id": "mathematics",
  "name": "Mathematics",
  "description": "One-sentence description of the subject for this grade.",
  "topics": [
    { "id": "algebraic-expressions", "title": "Algebraic Expressions", "term": 1 }
  ],
  "exams": [
    { "id": "june-paper-1", "title": "June Examination: Paper 1", "durationMinutes": 120, "totalMarks": 100 }
  ]
}
```

* `topics` are listed in teaching order. `term` is 1-4 (CAPS term in which the topic is taught).
* Every listed topic/exam must have a matching file, and every file must be listed.
* `title`, `term`, `durationMinutes` and `totalMarks` must match the values inside the topic/exam file.

## Topic file

```json
{
  "id": "algebraic-expressions",
  "title": "Algebraic Expressions",
  "term": 1,
  "summary": "One sentence shown in the topic list.",
  "notes": [ ...blocks ],
  "questions": [ ...questions ]
}
```

## Text markup (used in every text field)

Text is plain text (never HTML; `<` and `>` are shown literally). Supported markup:

| Markup             | Result                                                        |
|--------------------|---------------------------------------------------------------|
| `**bold**`         | **bold**                                                      |
| `*italic*`         | *italic* (also used for titles of literary works)            |
| `$x^2 + 1$`        | inline maths rendered by KaTeX (LaTeX syntax)                 |
| `\$`               | a literal dollar sign (e.g. `US\$5`); currency is normally `R` |
| `\n`               | line break; `\n\n` = paragraph break                          |

Because the files are JSON, LaTeX backslashes must be doubled: `"$\\frac{1}{2}$"`, `"$\\sqrt{x}$"`.
Chemistry: `$\\text{H}_2\\text{O}$` or simply `H₂O` with Unicode subscripts. Units: `$9{,}8\\ \\text{m·s}^{-2}$`
or plain `9,8 m·s⁻²`. South African conventions: decimal comma (`3,5`), space as thousands separator
(`12 500`), currency `R1 250,00`.

## Note blocks

| type         | fields                                                           |
|--------------|------------------------------------------------------------------|
| `heading`    | `text`                                                           |
| `paragraph`  | `text`                                                           |
| `list`       | `items` (array of text), optional `ordered` (boolean)            |
| `definition` | `term`, `text`                                                   |
| `example`    | optional `title`, `problem`, `steps` (array of text), optional `answer` |
| `callout`    | `label` (e.g. "Exam tip", "Remember", "Common mistake"), `text`  |
| `table`      | optional `caption`, `headers` (array), `rows` (array of arrays, same length as headers) |
| `formula`    | `latex` (display maths, no `$`), optional `caption`              |
| `passage`    | optional `title`, `text` (extract/poem/comprehension text; `\n` kept) |
| `diagram`    | `svg` (a complete `<svg ...>` string with `viewBox`), `caption`  |

Diagrams are rendered as images on a white background (no scripts run). Keep them simple: black strokes,
`font-family="sans-serif"`, a `viewBox`, no external references.

## Questions

Every question has: `id` (unique within the file), `type`, `prompt`, optional `marks`
(positive integer, default 1), optional `explanation` (shown after answering). Types:

| type        | extra fields | how it is marked |
|-------------|--------------|------------------|
| `mcq`       | `options` (2-6 texts), `answer` (0-based index of the correct option) | auto |
| `truefalse` | `answer` (true/false) | auto |
| `short`     | `answers` (array of accepted answers) | auto: case-insensitive, ignores extra spaces and trailing punctuation |
| `numeric`   | `answer` (number), optional `tolerance` (absolute, default 0.01), optional `unit` (shown next to the input) | auto: accepts `3,5`, `3.5`, `12 500`, `3/4`, `R150`; marks = full or zero. The prompt must say how to round (e.g. "correct to two decimal places") and `tolerance` must allow for it |
| `match`     | `pairs` (2-8 of `{ "left": text, "right": text }`) | auto: each pair is worth `marks / pairs.length` (so set `marks` = number of pairs) |
| `open`      | `memo` (marking guideline / model answer; required) | the learner compares with the memo and awards their own marks |

`mcq` options must be distinct; `match` right-hand values must be distinct and **plain text** (they are shown
in a drop-down list, so no `$maths$`, bold or italics; use Unicode such as x², √, π if needed). `short` answers should be
one or a few words (a term, a name, a value with no ambiguity). Use `open` for anything requiring
explanation, working or an essay; the memo should show how marks are allocated, e.g.
`"✓ identifies the metaphor (1)\n✓ explains the comparison (1)\n✓ links to theme (1)"`.

## Exam file

```json
{
  "id": "june-paper-1",
  "title": "June Examination: Paper 1",
  "durationMinutes": 120,
  "totalMarks": 100,
  "instructions": ["Answer ALL the questions.", "..."],
  "sections": [
    {
      "title": "QUESTION 1: Algebra",
      "instructions": "optional text",
      "context": [ ...optional note blocks shown before the questions (passage, table, diagram) ],
      "questions": [ ...questions (same format as above) ]
    }
  ]
}
```

* `totalMarks` must equal the sum of all question marks.
* Questions are numbered automatically (1.1, 1.2, ... within section 1).
* Question `id`s must be unique within the exam.
