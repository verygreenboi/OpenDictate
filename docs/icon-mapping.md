# Lucide icon mapping

Every glyph in the UI, and what it becomes. Source of truth for the icon swap.

The app currently mixes three icon systems, which is the real reason it reads
as untidy: hand-drawn vector paths, colour emoji, and Unicode symbols. Emoji in
particular render in full colour with their own style, nothing like the
monochrome drawn icons beside them.

## How

Lucide ships `lucide.ttf` in the `lucide-static` npm package with a JSON
codepoint map. The font joins the existing families in `theme.rs` as a
fallback, so a Lucide codepoint inside an ordinary string renders as an icon
with no change to layout. That keeps the diff small: most sites are a single
character swap inside a label.

Glyphs get named constants (`icon::SEARCH`) rather than raw escapes, so call
sites stay readable.

## 1. Hand-drawn, in `draw_icon` (views.rs)

These keep the `draw_icon(painter, name, centre, colour)` signature, so all 12
call sites are untouched. Only the bodies change, from drawn paths to a glyph.

| Name | Where | Lucide |
|---|---|---|
| `home` | Home nav | `house` |
| `clock` | History nav | `history` |
| `book` | Dictionary nav | `book-open` |
| `mic` | audio button, Models + Audio tabs | `mic` |
| `gear` | Settings | `settings` |
| `spark` | Personalization | `sparkles` |
| `help` \| `info` | About button and tab | `circle-help` |
| `close` | close and remove buttons | `x` |

## 2. Settings section headers (settings_view.rs)

| Glyph | Section | Lucide |
|---|---|---|
| 🎤 | Speech to text | `mic` |
| ✨ | Text clean-up | `wand-sparkles` |
| 🧪 | Try it | `flask-conical` |
| ⌨ | Shortcuts | `keyboard` |
| 🌐 | Language | `globe` |
| 📋 | Output | `clipboard` |
| 🎙 | Microphone | `mic-vocal` |
| 🔒 | Permission | `lock` |
| 🎨 | Tone | `palette` |
| 🧹 | Clean-up | `eraser` |
| 📝 | Your own instructions | `pencil-line` |
| ⏱ | Time saved | `timer` |
| 👁 | What your text model is told | `eye` |
| 📁 | Your data | `folder` |
| 📊 | Totals | `chart-column` |

## 3. Inline glyphs in buttons and labels

| Glyph | Where | Lucide |
|---|---|---|
| 🎤 | "Start dictating" | `mic` |
| ■ | "Stop & paste" | `square` |
| ⬆ | "Share" | `share-2` |
| ↻ | "Retry" | `rotate-ccw` |
| 🔍 | Dictionary search field | `search` |
| ⓘ | error detail | `info` |
| ✦ | auto-added word | `sparkles` |
| ✎ | manual word | `pencil` |
| × | remove | `x` |
| ✔ | found / in use / running | `check` |
| ✖ | not found / not running | `x` |
| ⚠ | warnings | `triangle-alert` |
| 🔒 | "Stored only on this Mac", "Your data stays yours" | `lock` |
| 🗄 | "Keep history" | `archive` |
| ⋯ | overflow menu buttons | `ellipsis` |

## Staying as they are

The keyboard modifiers — ⌥ ⇧ ⌃ ⌘ — are not icons. They are the standard Mac
key symbols and every Mac user already reads them; Lucide's versions would be
worse. Same for the `F13–F19` key labels.

## Risk

The drawn icons are pixel-tuned for the sizes they appear at. Lucide is a 24px,
2px-stroke grid scaled down to roughly 13–16px here, so some may read lighter
than what they replace. Screenshots before and after, at real size, decide
whether any need their stroke or size nudged.
