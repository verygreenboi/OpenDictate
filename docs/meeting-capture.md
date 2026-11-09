# Meeting mode, part one: capture and storage

What a meeting recording is, where it lives, and how the audio gets in. No
summarising here — that is part two, and it is designed against whatever real
transcripts this produces rather than against a guess.

v1 is useful on its own: press a key, the meeting is recorded and transcribed
as it runs, and it is there afterwards to read and search.

## What it does

A fourth action, `Meeting`, with its own shortcut. Unlike the other three it
is always a toggle: press to start, press to stop, hold-to-talk does not apply
to something that runs for an hour.

While it runs: the menu bar icon shows recording, the indicator pill shows the
elapsed time instead of a level meter, and a Meetings page in the side panel
shows the transcript filling in. Stopping leaves a meeting you can reopen.

## The one hard problem: whose audio

The app records the microphone. In a meeting that is you and nobody else — the
other voices come out of the speakers, and a recorder that only hears one side
of a call is a different product from the one described above.

Three ways to get the rest:

**A virtual audio driver (BlackHole).** The usual answer. It needs a kernel
extension installed by hand, it moves the system output into a device the user
has to manage, and it is the kind of setup step that loses people.

**ScreenCaptureKit.** Can capture audio alone, but it is the screen-recording
API and asks for the screen-recording permission. For an app whose pitch is
that nothing leaves the Mac, asking to record the screen in order to hear a
call is a bad trade.

**A Core Audio process tap.** Taps system output directly, no driver, no
screen. This is the one to use.

### What was checked

- `AudioHardwareCreateProcessTap`, `AudioHardwareDestroyProcessTap`,
  `CATapDescription`, `kAudioTapPropertyFormat/UID/Description`,
  `kAudioAggregateDeviceTapListKey` and `kAudioSubTapUIDKey` are all present in
  `objc2-core-audio` 0.3.2, which is **already in the dependency tree** via
  winit. No new crate, no C shim.
- The development Mac is macOS 27.0.1, well past the floor.

### What was not checked, and must be before anything is built

- **The API floor is macOS 14.2–14.4**; this app's `LSMinimumSystemVersion` is
  11.0. Taps have to be resolved at runtime with a mic-only fallback below
  that, or the bundle's floor has to move and leave those users behind.
- **Permission.** Tapping needs the user's Screen & System Audio Recording
  consent, and reports say the prompt appears when the aggregate device
  *starts*, not when the tap is created — so the failure mode is a meeting
  that records silence from everyone but you until it is granted.
- **It is reported to fail outright** in some setups with
  `kAudioHardwareIllegalOperationError` and no prompt at all.

**So step one is a spike, not a feature:** a throwaway binary that creates a
tap, builds an aggregate device from it plus the default input, starts it, and
writes ten seconds to a WAV. If that file has both sides of a call in it, the
rest of this document is buildable. If it does not, meeting mode ships
mic-only in v1 and says so plainly in the UI.

### If the spike passes

One aggregate device containing the tap and the microphone, read as a single
stream, so mixing is Core Audio's problem rather than ours. The existing
`audio::Recording` grows a second constructor; everything downstream sees the
same samples it does today.

## Capture pipeline

A meeting is too long to hold in memory and too long to transcribe in one go.

1. Record into **30-second chunks**, written to disk as they close.
2. Transcribe each closed chunk with whisper.cpp while the next one fills.
3. Append the result to the transcript with its offset from the start.

This keeps memory flat however long the meeting runs, puts a transcript on
screen while it is still going, and means a crash costs at most the last
chunk.

Chunking on a fixed clock cuts words in half at the boundary. Two things fix
most of it: a **2-second overlap** between chunks with the duplicate text
dropped on append, and moving each cut to the nearest silence inside a short
window. The second needs the level data the recorder already computes for the
meter.

large-v3-turbo runs far faster than real time on Apple silicon, so
transcription keeps up with the meeting on one worker. One chunk at a time,
queued — not a thread per chunk.

## Storage

```
~/Library/Application Support/Open Dictate/meetings/<started-at>/
  meta.json        title, started, ended, duration, source (mic | mic+system)
  transcript.jsonl one line per segment: { at_ms, secs, text }
  audio/000.wav …  chunks, deleted as they are transcribed
```

JSONL rather than one JSON document: a meeting in progress is appended to
every half minute, and a crash truncates a line instead of corrupting the
file.

**Audio is deleted once transcribed**, by default. 16 kHz mono 16-bit is
1.9 MB per minute — a 90-minute meeting is 170 MB, and a week of them fills a
disk. A "keep the recording" setting can hold them for people who want to
re-run a better model later; that setting is where FLAC earns its place
(roughly half the size, lossless, and whisper.cpp reads it).

Meetings are separate from history. History entries are things that were
pasted somewhere; a meeting was never pasted anywhere, and mixing them makes
both lists worse.

## Not in v1

Summaries, open-question tracking, actions — part two, and the reason for
part one. Also out: speaker labels (diarisation is a separate model),
calendar integration, video, and anything that sends a meeting off this Mac.

## Open questions

- Does the tap prompt once and stay granted, like the microphone, or per
  session? Changes whether meeting mode can be trusted to start unattended.
- What happens to a tap when output switches mid-meeting — headphones
  unplugged, AirPods connecting? Likely needs a device-change listener and a
  new tap, without dropping the recording.
- Does a meeting need a title before it starts, or is "Monday 14:03" plus a
  rename afterwards enough? Second one, probably: naming a thing before it
  happens is a step in the way.
