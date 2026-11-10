# Record the meeting microphone as its own stream

## Why

Meetings record your side and the call's side through one private Core Audio aggregate device (`src/tap.rs`): the system default input as a sub-device, plus a process tap on everything the Mac plays. One IO callback delivers both on one clock, which is why the two sides line up frame for frame.

That design has failed three ways in real calls:

1. **Screen sharing turns your mic down.** In a measured call, your YOU stretches had a median peak of 0.061 before sharing (128 stretches) and 0.018 while sharing (24 stretches). Reading the same interface directly at the same moment (AVAudioEngine on input 1) gave 0.087. Your voice never came back on the call side ("purple elephant" appeared on neither side). Something about sharing reduces only the copy of the mic inside the aggregate. We don't know exactly what, and we don't need to: the direct read was unaffected.
2. **The mic can drop out of the aggregate** mid-meeting, which is already recorded in CLAUDE.md.
3. **The meeting can't choose the mic.** It always takes the system default input and has to guess which channels of a multi-input interface are the mic. The Microphone choice in Settings › Audio input is ignored for meetings.

## Goal

Your side is captured from the microphone itself, at its real level, whatever the call app does with screen sharing, and it stays lined up with the call side well enough for the cutter's 20 ms frames.

Success, measured in a real call on the Universal Audio interface: the median peak of the YOU stretches while sharing is at least half of the median before sharing, and what you say while sharing is transcribed as YOU.

## Non-goals

- Echo cancellation, or removing the call's audio from your mic on speakers. The existing "both" handling stays.
- Telling participants apart within the call side. That is the separate voice-print design.
- A channel picker UI for interfaces. Today's rule (the louder of the first two channels) stays the default; a picker can come later.

## Anti-goals

These would make it a failure even if it works:

- YOU and THEM drifting apart over a long meeting until turns are cut in the wrong places. One hour of a 50 ppm clock difference is 180 ms, nine frames.
- Breaking the "no system audio permission" message, or the "your microphone was not captured" note on the meeting.
- More noise reaching Whisper. The guard thresholds and their measured reasons in CLAUDE.md stay as they are.

## Design

### Two capture paths

**Call side:** the aggregate keeps only the process tap, with no microphone sub-device. Everything else in `SystemCapture` (permission handling, pause, drain) stays.

**Your side:** a separate input stream on the microphone, built on the same cpal setup dictation uses in `audio::Recording` (device lookup by name, sample-format conversion), not new Core Audio code.
- **Refactor needed:** `Recording`'s sample handling is a nested `push` that averages channels and applies gain, so it has to be factored out.
- **Channel pick:** each callback takes the louder of the first two channels (`tap::MIC_INPUTS` and `mix_mic`, made public), never an average, so the #11 rule moves here unchanged.

**Check before PR 1 depends on it:** today's no-mic fallback already builds a tap-only aggregate. It has an empty sub-device list and a main sub-device that isn't in that list. Apple's examples list the output device, and our version has only ever run as a fallback. The first step of the plan is to confirm it on your interface with `--tap-check`.

**Which microphone:** in PR 1, the system default input, so behaviour matches today. In PR 2, `settings.microphone`, with empty meaning the system default, the same as dictation. `input_gain` is not applied to meetings; the guard assumes raw levels.

### One rate and one timeline

- **Rate:** the cutter keeps running at the call stream's rate (`cap.rate`), as it does today; meetings only convert to 16 kHz when writing a stretch (`to_16k`). Only the mic stream is resampled, to the call stream's rate, as it is drained, so the guard thresholds measured at device rate stay valid. Use linear interpolation when going up in rate. When going down (e.g. a 96 kHz interface into 48 kHz), average neighbouring samples first, as `to_16k` does, to avoid aliasing.
- **Starting together:** each stream records the host time of its first sample. cpal 0.18 gives `capture` as host time in ns, less its buffer-latency estimate. The tap's IO proc receives `inInputTime`. The stream that started earlier has its head trimmed so both begin at the same instant. The two clocks are corrected differently, so a fixed offset of a few ms remains, which is under one 20 ms frame.
- **Drift:** at each drain (every 250 ms), compare each stream's sample count against the host time it covers. The call stream is the reference. When the mic is more than 10 ms ahead or behind, drop or repeat a single mic sample per drain until it is within 2 ms. 50 ppm at 48 kHz is about 2.4 samples a second, so one sample per drain keeps up without an audible click.
- **Waiting for the slower stream:** the cutter only ever receives equal lengths of YOU and THEM, because `Cutter::push` pads an empty side with zeros, and a late mic callback would otherwise put the two sides out of step. The aligner holds whatever one stream has beyond the other. If the other stream delivers nothing for 500 ms, it pads the missing side with silence and writes a line to `levels.log`, so a dead mic never stalls THEM.

### When the mic fails

- **Mic won't open** (no input device): the meeting starts call-side only, and the meeting's source says so.
- **No microphone permission:** cpal usually opens the mic and delivers zeros rather than failing. Today's `heard_you` check catches this and marks the meeting "your microphone was not captured". That stays.
- **Mic stream errors or goes quiet mid-meeting** (unplugged, AirPods switched): your side becomes silence, so stretches are labelled THEM. `levels.log` gets a line, and the live warning says the microphone stopped. Reopening is out of scope for now.
- **`split_ok`:** true whenever the meeting has a mic stream, including after it dies. It is false only when the meeting started without one. In that case everything goes through the cutter's existing `identical || !split_ok` path as "both", exactly as today.
- **Source text:** the meeting's source line ("You and everyone on the call") comes from whether the mic stream opened, not `cap.has_mic`.
- **Ending a meeting:** the final drain stops both the tap and the mic stream before the last read, following the CLAUDE.md rule, so neither keeps adding audio after the end.

### Diagnostics

The one thing missing while debugging today's call.

- **At start**, `levels.log` names the mic device, its rate and channel count, and which channel is in use.
- **Every 30 s**, it records the peak of each side and the current offset between the streams in milliseconds.

## Testing

Test-first and outside-in, as the project does.

**Unit tests:**
- Resampling the mic to the call stream's rate keeps a tone's frequency and level, both going up (44.1 to 48 kHz) and coming down (96 to 48 kHz).
- A dead mic stream produces THEM-only stretches after the 500 ms hold, and THEM never stalls.
- Start alignment trims the earlier stream by the timestamp difference.
- Drift: a mic stream running 100 ppm fast over a simulated 60 minutes stays within 10 ms of the call stream.
- Unequal arrivals are held, not dropped.
- Channel pick: the existing `mix_mic` tests move to the new stream.

**Real-device checks:**
- `--meeting-check` with `say` playing: both sides are labelled as today.
- **Your check in a real call:** talk, share your screen, talk, stop sharing. Compare the YOU medians from `levels.log` against the success measure above.

## PR sequence

Each PR can ship on its own; the mechanism comes before the behaviour.

1. **Separate mic stream and alignment.** It uses the system default input, so the only behaviour change is that sharing no longer turns you down. It also adds the diagnostics lines.
2. **Meetings use the Microphone chosen in Settings › Audio input**, and the setting's description says so.
3. **Dictation picks the louder of the first two channels instead of averaging** (`audio.rs:76`). On a 16-channel interface, dictation currently also reaches Whisper at 1/16 level.

## Left to the builder

- Where the aligner lives: a new `src/align.rs` is my suggestion.
- Whether resampling happens in the stream callbacks or at drain. Drain is suggested, to keep the callbacks lean.

## My calls

Change any of these before planning.

- PR 1 keeps the system default mic; the settings mic waits for PR 2.
- `input_gain` stays out of meetings.
- Nothing tries to reopen a mic that drops mid-meeting.
- The call stream is the reference clock.
- The cutter stays at device rate rather than moving to 16 kHz, so the measured guard thresholds stay valid.
- A mic that dies mid-meeting means THEM-only stretches, not "both".
- A late stream is held for at most 500 ms, then padded with silence.
