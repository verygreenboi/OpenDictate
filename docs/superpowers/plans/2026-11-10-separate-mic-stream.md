# Separate Meeting Mic Stream Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Meetings capture your side from the microphone as its own cpal stream, lined up with a tap-only system-audio capture, so screen sharing can no longer turn your side down.

**Architecture:** `SystemCapture` keeps only the process tap and reports the host time of its first sample. A new `MicStream` (cpal) takes the louder of the first two input channels. A new `align` module resamples the mic to the tap's rate and hands the cutter equal-length, time-aligned YOU and THEM slices. To do that it trims the start, corrects drift one sample at a time, and pads a stalled side after 500 ms.

**Tech Stack:** Rust 2024, cpal 0.18, objc2 Core Audio bindings already in `src/tap.rs`, egui_kittest for the one UI check.

**Spec:** `docs/superpowers/specs/2026-11-10-separate-mic-stream-design.md`

## Global Constraints

- The cutter keeps running at the call stream's rate (`cap.rate`); meetings convert to 16 kHz only when writing a stretch (`to_16k`).
- Do not change `worth_transcribing`, `VOICED_RMS`, `NOISE_K`, `SIDE_RATIO`, `PAUSE_FRAMES`, `TURN_FRAMES` or any other `cut.rs` constant.
- The mic side is the louder of the first `MIC_INPUTS` (= 2) channels per callback, never an average.
- `input_gain` is not applied to meetings.
- The call stream is the reference clock. Drift correction kicks in above 10 ms and stops below 2 ms, moving one mic sample per drain.
- Hold limit: a side that delivers nothing for 500 ms is padded with silence, and `levels.log` gets a line.
- `split_ok` is true whenever the meeting opened a mic stream, even after it dies, and false only when the meeting started without one.
- Diagnostics in `levels.log`: one line at start naming the mic device, rate, channel count and the channel in use; then every 30 s, each side's peak and the stream offset in ms.
- Final drain: pause the tap and the mic stream before the last read.
- The meeting loop still drains every 250 ms.
- Project rules: tests first, outside-in; intent goes in test names; no comments that describe code; one PR per numbered PR below; no AI trailers.

## Review Focus

1. **Mic and tap at different rates** (44.1 kHz interface, 48 kHz tap): your speech should be labelled at the right time and pitch. Pinned by Task 1 and the integration test in Task 5.
2. **Mic unplugged or AirPods switched mid-meeting:** THEM keeps flowing and later stretches are THEM. Pinned by Task 2, `a_dead_mic_is_padded_after_500ms_and_them_never_stalls`.
3. **No input device at start:** the meeting still records the call, the source says so, and stretches take today's "both" path. Pinned by Task 5, `a_meeting_without_a_mic_records_the_call_as_both`.
4. **Hour-long meeting:** YOU and THEM stay within 10 ms. Pinned by Task 2's drift test.
5. **Mic permission denied** (cpal delivers zeros): the meeting is marked "your microphone was not captured". Pinned by Task 5, `a_silent_mic_marks_the_meeting_as_not_captured`.

---

## PR 1: Separate mic stream and alignment

### Task 1: Streaming resampler

**Files:**
- Create: `src/align.rs` (declare `mod align;` in `src/main.rs`)

**Interfaces:**
- Produces: `pub struct Resampler`, `Resampler::new(from: f64, to: f64) -> Self`, `Resampler::push(&mut self, input: &[f32]) -> Vec<f32>`. It keeps its fractional position across calls, so chunked input gives the same output as one call.

- [ ] **Step 1: Write the failing tests** in `src/align.rs` (`#[cfg(test)] mod tests`)

```rust
fn tone(rate: f64, hz: f64, secs: f64, level: f32) -> Vec<f32> // helper: sine at `level`
fn zero_crossings_per_sec(v: &[f32], rate: f64) -> f64          // helper

#[test] fn upsampling_44k1_to_48k_keeps_a_tones_pitch_and_level()
    // 1 kHz tone at 0.3 for 1 s at 44_100 -> Resampler::new(44_100.0, 48_000.0)
    // assert output len within 1 sample of 48_000; crossings/sec within 1% of 2_000; peak within 0.01 of 0.3
#[test] fn downsampling_96k_to_48k_keeps_a_tones_pitch_and_level()
    // same with 96_000 -> 48_000
#[test] fn chunked_input_resamples_the_same_as_one_push()
    // 44.1k tone pushed in 441-sample chunks equals one push within 1e-6 per sample
#[test] fn equal_rates_pass_through_unchanged()
```

- [ ] **Step 2: Run** `cargo test align::` and confirm it fails to compile with `Resampler` undefined.
- [ ] **Step 3: Implement `Resampler`.** Going up in rate, interpolate linearly. Going down, first average each source window of `from/to` samples (the box filter `to_16k` uses), then interpolate. When `from == to`, copy the input through.
- [ ] **Step 4: Run** `cargo test align::` and confirm all 4 pass.
- [ ] **Step 5: Commit** "Resample the meeting mic to the call stream's rate".

### Task 2: Aligner

**Files:**
- Modify: `src/align.rs`

**Interfaces:**
- Consumes: `Resampler` (Task 1) is not used here. The aligner takes mic samples already at `rate`.
- Produces:
  - `pub struct Aligner`
  - `Aligner::new(rate: f64) -> Self`
  - `push_call(&mut self, samples: &[f32], first_host_ns: Option<u64>)`
  - `push_mic(&mut self, samples: &[f32], first_host_ns: Option<u64>)`
  - `take(&mut self, now_ns: u64) -> (Vec<f32>, Vec<f32>)` returns `(mine, theirs)` of equal length
  - `offset_ms(&self) -> f32`
  - `padded(&mut self) -> Option<&'static str>` returns `"mic"` or `"call"` once each time a side is padded after the hold
  - `const HOLD_MS: u64 = 500; const DRIFT_START_MS: f32 = 10.0; const DRIFT_STOP_MS: f32 = 2.0;`

- [ ] **Step 1: Write the failing tests**

```rust
#[test] fn the_stream_that_started_earlier_is_trimmed_so_both_begin_together()
    // rate 48k; call first_host_ns = 1_000_000_000, mic first_host_ns = 1_020_000_000 (20 ms later)
    // push 0.5 s each; take(): mine and theirs equal length, and theirs[0] is the call's sample at index 960
#[test] fn unequal_arrivals_are_held_not_dropped()
    // push 0.3 s call, 0.1 s mic; take() returns 0.1 s each; push 0.2 s mic; take() returns the next 0.2 s of both
    // concatenated outputs equal the inputs exactly
#[test] fn a_mic_running_100ppm_fast_stays_within_10ms_over_an_hour()
    // simulate 14_400 drains of 250 ms; the mic delivers 100 ppm more samples per host-time span than the call
    // after every take, offset_ms().abs() <= 10.0; at the end <= 2.0 + one drain's worth
#[test] fn drift_moves_at_most_one_mic_sample_per_take()
#[test] fn a_dead_mic_is_padded_after_500ms_and_them_never_stalls()
    // mic pushes 1 s then nothing; call keeps pushing 250 ms per take with now_ns advancing
    // within 2 takes after now passes the last mic arrival + HOLD_MS, take() returns the call's samples with mine all zeros
    // padded() == Some("mic") once
#[test] fn a_stalled_call_is_padded_the_same_way()
```

- [ ] **Step 2: Run** `cargo test align::` and confirm the new tests fail.
- [ ] **Step 3: Implement `Aligner`.**
  - Keep a buffer per side, plus each side's first host time and sample count.
  - **Start trim:** on the first `take` where both start times are known, drop `(later - earlier) * rate` samples from the earlier side.
  - **Drift:** each side's clock ratio is samples counted divided by `(last arrival host time - first host time) * rate`. Turn the mic's deviation from the call into ms. Above `DRIFT_START_MS`, drop one mic sample (mic fast) or repeat one (mic slow) per `take`, and keep correcting until the deviation is under `DRIFT_STOP_MS`.
  - **Hold:** when one side has had no arrival for `HOLD_MS` of `now_ns`, pad it with zeros to the other side's length.
- [ ] **Step 4: Run** `cargo test align::` and confirm everything passes.
- [ ] **Step 5: Commit** "Line up the mic and call streams on the host clock".

### Task 3: MicStream

**Files:**
- Create: `src/mic.rs` (declare `mod mic;` in `src/main.rs`)
- Modify:
  - `src/tap.rs`: make `MIC_INPUTS` and `mix_mic` `pub(crate)` and move their 5 tests to `src/mic.rs`
  - `src/audio.rs`: factor the sample-format conversion out of `Recording::start` into `pub(crate) fn input_stream(device: &cpal::Device, on_data: impl FnMut(&[f32], u64) + Send + 'static, on_error: impl FnMut(String) + Send + 'static) -> Result<(cpal::Stream, u32 /*rate*/, usize /*channels*/), String>`. It delivers interleaved f32 and the capture host time in ns. `Recording` uses it with its existing averaging and gain unchanged in this PR.

**Interfaces:**
- Produces:
  - `pub(crate) fn pick_channels(interleaved: &[f32], channels: usize) -> Vec<f32>`: deinterleaves up to `MIC_INPUTS` channels and returns `mix_mic` of them
  - `pub struct MicStream`
  - `MicStream::start(name: &str) -> Result<Self, String>`, where `""` means the system default
  - `drain(&self) -> (Vec<f32>, Option<u64>)`: samples at the device rate, and the host ns of the first sample since the last drain
  - `rate(&self) -> f64`
  - `info(&self) -> String`: device name, rate, channel count and the channel in use
  - `pause(&self)`
  - `failed(&self) -> Option<String>`: set by the stream's error callback

- [ ] **Step 1: Write the failing tests** in `src/mic.rs`: the 5 `mix_mic` tests moved from `tap.rs`, plus:

```rust
#[test] fn a_voice_on_channel_0_of_16_interleaved_reaches_full_level()
    // 480 frames x 16 ch; ch0 = 0.3 tone, ch2/3/6/7 = 0.47 loopback tone, rest 0.001 noise
    // pick_channels(.., 16) == ch0 samples exactly
#[test] fn a_stereo_mic_takes_the_louder_channel()
#[test] fn a_mono_mic_passes_through()
```

- [ ] **Step 2: Run** `cargo test mic::` and confirm `pick_channels` is undefined.
- [ ] **Step 3: Implement** `pick_channels`, `audio::input_stream` and `MicStream` on top of them. Take the capture time from cpal's `InputCallbackInfo::timestamp().capture`, which is host time in ns on macOS (cpal 0.18).
- [ ] **Step 4: Run** `cargo test` and confirm the whole suite passes, with dictation unchanged.
- [ ] **Step 5: Commit** "Open the meeting mic as its own stream".

### Task 4: Tap-only capture with a start time

**Files:**
- Modify: `src/tap.rs`

**Interfaces:**
- Produces: `Chunk { mine, theirs, first_host_ns: Option<u64> }`, where `first_host_ns` is the host ns of the first sample in `theirs` since the last drain, from `inInputTime.mHostTime` converted with the mach timebase. Meetings now call `SystemCapture::start(None)`. `tap::check` keeps `default_input()`.

- [ ] **Step 1: Write the failing test.**

```rust
#[test] fn host_ticks_convert_to_nanoseconds_with_the_timebase()
    // pub(crate) fn host_to_ns(ticks: u64, numer: u32, denom: u32) -> u64
    // (125, 3) maps 3_000 ticks to 125_000 ns; (1, 1) is identity
```

- [ ] **Step 2: Run** `cargo test tap::` and confirm it fails.
- [ ] **Step 3: Implement.** Add `host_to_ns`, and record the first `inInputTime` host time into the `Sink` per drain window. Before anything relies on the tap-only aggregate, run the real-device check: `PATH=<dir with a no-op afplay>:$PATH ./target/release/open-typeless --no-bundle --tap-check` with `SystemCapture::start(None)`, while `say` plays. Expect `VERDICT: system audio captured.` and `them` > 0.05. If it fails, add the default output device to the sub-device list (Apple's layout) and run the check again.
- [ ] **Step 4: Run** `cargo test` and confirm everything passes. The real-device check passes too.
- [ ] **Step 5: Commit** "Capture only the call through the tap and time its first sample".

### Task 5: Wire meetings to the two streams

**Files:**
- Modify: `src/meeting.rs` (`Session::start`, `run`, `finish`)

**Interfaces:**
- Consumes: `MicStream` (Task 3), `SystemCapture::start(None)` and `Chunk::first_host_ns` (Task 4), `Resampler` and `Aligner` (Tasks 1–2).
- Produces:
  - `fn source_line(mic_opened: bool) -> &'static str`: returns "You and everyone on the call" or "Everyone on the call"
  - `fn step(cutter: &mut Cutter, aligner: &mut Aligner, resampler: &mut Option<Resampler>, call: Chunk, mic: Option<(Vec<f32>, Option<u64>)>, now_ns: u64, split_ok: bool) -> Vec<Stretch>`: one loop iteration, pure enough to test
  - `fn diagnostics_line(mine_peak: f32, theirs_peak: f32, offset_ms: f32) -> String`

- [ ] **Step 1: Write the failing tests** in `src/meeting.rs` through `step`, using recorded fixtures (`tests/fixtures/question.wav` as the call, `yes.wav` as the mic):

```rust
#[test] fn a_44k1_mic_and_48k_call_produce_you_and_them_stretches_in_order()
    // call = question.wav upsampled to 48k, then 2 s silence; mic = 2 s silence then yes.wav at 44.1k
    // feed 250 ms per step with consistent host times; voices == ["them", "you"]
#[test] fn a_meeting_without_a_mic_records_the_call_as_both()
    // mic None, split_ok false -> voices == ["both"]; source_line(false) == "Everyone on the call"
#[test] fn a_silent_mic_marks_the_meeting_as_not_captured()
    // all-zero mic for the whole run -> heard_you stays false (use the same rms > 0.001 rule as today)
#[test] fn the_diagnostics_line_names_both_peaks_and_the_offset()
    // diagnostics_line(0.061, 0.17, 3.2) == "levels you 0.061 them 0.170 offset 3.2 ms"
```

- [ ] **Step 2: Run** `cargo test meeting::` and confirm the new tests fail.
- [ ] **Step 3: Implement.**
  - **`Session::start`:** open `MicStream::start("")`. On error, carry on without a mic and log it. Then `SystemCapture::start(None)`. `source = source_line(mic.is_some())`. Log `mic.info()` at start.
  - **`run`:** each 250 ms tick calls `step`. It logs `diagnostics_line` every 30 s, logs and warns once on `mic.failed()` ("Your microphone stopped"), and logs `aligner.padded()`. `heard_you` is unchanged.
  - **`finish`:** pause the tap and the mic, drain both, run `step` once more, then `cutter.flush()`.
- [ ] **Step 4: Run** `cargo test`, then the real-device checks:
  - `./target/release/open-typeless --no-bundle --meeting-check 30` while `say` plays two lines. Expect them labelled `them`, and the start line names the mic.
  - The user's call test: talk, share the screen, talk, stop sharing. The median YOU peak while sharing should be at least half the median before.
- [ ] **Step 5: Commit** "Record meetings from a separate mic stream lined up with the call", and record the decision in CLAUDE.md's "Meeting capture: decisions", with the measured sharing numbers (0.061 to 0.018 in the aggregate, against 0.087 read directly).

## PR 2: Meetings use the chosen microphone

### Task 6: settings.microphone for meetings

**Files:**
- Modify:
  - `src/meeting.rs`: `MicStream::start(&s.microphone)`
  - `src/settings_view.rs:710`: the row description

- [ ] **Step 1: Write the failing test** in the `settings_view` kittest tests: `the_microphone_row_says_it_is_used_for_meetings`. It asserts the label "Which microphone records your voice, in dictation and meetings." exists.
- [ ] **Step 2: Run** it and confirm it fails.
- [ ] **Step 3: Implement** the one-line change in each file.
- [ ] **Step 4: Run** `cargo test` and confirm everything passes.
- [ ] **Step 5: Commit** "Use the chosen microphone in meetings".

## PR 3: Dictation stops averaging interface channels

### Task 7: Recording picks channels like meetings

**Files:**
- Modify: `src/audio.rs` (`Recording`'s callback)

**Interfaces:**
- Consumes: `mic::pick_channels` (Task 3).

- [ ] **Step 1: Write the failing test** `dictation_on_a_16_channel_interface_keeps_the_mic_at_full_level`. Feed `Recording`'s extracted frame handler 16-channel interleaved data with 0.3 on ch0, and assert the stored peak is 0.3 times gain, not 0.3/16.
- [ ] **Step 2: Run** it and confirm it fails at about 0.019.
- [ ] **Step 3: Implement.** Replace the per-frame average with `pick_channels`, then apply gain and clamp as today.
- [ ] **Step 4: Run** `cargo test`. Then dictate one sentence on the UA interface and confirm it transcribes.
- [ ] **Step 5: Commit** "Keep a multi-input interface's mic at full level in dictation".
