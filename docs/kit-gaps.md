# What the settings screen needed that Paper & Ink didn't have

**All ten are now in the kit, under its Forms section, and applied in the app.**
Kept as the record of what was missing and why each was treated the way it was.

Taken from the five settings tabs, counted against the kit's CSS. The kit was
built for a game — score ledgers, life bars, an expanding purchase menu — so
it is strong on surfaces and buttons and has almost nothing for forms. Every
form control in settings is still a stock egui widget wearing whatever the
global style gives it.

Ordered by how often the screen uses them.

## 1. Form row — 27 uses

A title, a line of helper text under it, and a control on the right. It is the
spine of every settings tab and the kit has no equivalent at all.

Nearest in the kit: nothing. `.krow` and `.lrow` are read-only list rows.

Paper & Ink treatment: no border of its own — rows sit inside an already
outlined panel, and a box per row would repeat the segmented-control mistake.
A hairline between rows, title in body 700, helper in 600 at 0.76 opacity, the
control right-aligned in a fixed column.

## 2. Section header — 15 uses

An icon, a title, and a rule across the remaining width.

Nearest in the kit: `.kit-h`, but that is the demo page's own heading, not a
component.

Paper & Ink treatment: icon at 19px, title in the display face, a 1.5px
hairline. Close to what the app already draws, just not written down.

## 3. Text field — 13 uses

Single-line input: model names, URLs, API keys, language.

Nearest in the kit: nothing. The only styled `input`s are the hidden checkbox
and radio.

Paper & Ink treatment: 2.5px ink edge, `--r-square`, surface fill, inset
shadow rather than an offset one — a field is a hole, not a raised thing. Ink
caret, and the focus ring the kit already uses (3px `--y2`).

## 4. Status line — 11 uses

"Found at /usr/local/bin", "Not running", "No sound is arriving". An icon plus
a short state, tinted green, red or yellow.

Nearest in the kit: nothing, though `.mtag` tints a label.

Paper & Ink treatment: icon plus text, colour from the three-colour semantic
the kit already states — blue settled, yellow noticed, red wrong.

## 5. Select — 5 uses

Microphone, language, keep-history, model pickers.

Nearest in the kit: `.menu`, an expanding list, which is the open state but
not the closed control.

Paper & Ink treatment: a secondary button with a caret that opens `.menu`.
Most of this exists; it needs joining up.

## 6. Textarea — 2 uses

Custom instructions, and the "try it" input.

Paper & Ink treatment: the text field, taller, with the same inset edge.

## 7. Slider — 2 uses

Input gain, typing speed.

Nearest in the kit: `.fee-track` is a meter you read, and the fee panel has a
stepper, but there is nothing you drag.

Paper & Ink treatment: an ink-edged track with a square ink thumb carrying the
small hard shadow, pressing into the track as it moves.

## 8. Progress with a label — 2 uses

Model downloads, showing percent and status.

Nearest in the kit: `.meter` and `.card-meter` have the bar; neither carries a
label or a number.

## 9. Link — 1 use

"Check models".

Paper & Ink treatment: ink text with a 2px underline that thickens on hover.

## 10. Disabled

The kit disables `.btn` and the form controls at 0.28 and 0.4 opacity with no
shadow. It is two one-off rules rather than a stated convention, and the app
has four `add_enabled` sites relying on egui's own graying.

## Not gaps

Buttons, icon buttons, checkbox, radio, segmented control, choice cards,
notices, cards, panels, the side panel, key chips and meters are all in the
kit and in use.

Tooltips are in the kit as `.tip` but the app still uses egui's native
`on_hover_text` in two places, so that is an application gap, not a library
one.
