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

### What the spike found

Run on macOS 27.0.1 against the real API, playing a system sound at itself
and recording the result:

```
AudioHardwareCreateProcessTap      -> 0   (tap id 152)
AudioHardwareCreateAggregateDevice -> 0   (device 153)
tap format                            48000 Hz, 2 ch, 32 bit float
AudioDeviceCreateIOProcID          -> 0
AudioDeviceStart                   -> 0
19.0 s captured, peak 0.359, rms 0.023
```

The envelope shows four discrete bursts separated by silence, matching the
sound as it played, so this is system output and not noise on an open line.
**System audio capture works, with no virtual driver and no screen
recording.** The reported `kAudioHardwareIllegalOperationError` did not
appear.

The sequence that worked, for the implementation to follow:

1. `CATapDescription::initStereoGlobalTapButExcludeProcesses` with an empty
   exclude list, `setPrivate(true)`, mute behaviour `Unmuted` — so the
   meeting is still audible while it is recorded.
2. `AudioHardwareCreateProcessTap`, then read the description's `UUID`.
3. A private aggregate device whose `kAudioAggregateDeviceTapListKey` holds
   one sub-tap with that UUID, and whose main sub-device is the current
   default output's UID.
4. `AudioDeviceCreateIOProcID` + `AudioDeviceStart`; samples arrive in the
   IO proc's input buffer list as interleaved f32.

### Permission, settled

The first spike ran from a terminal that already holds screen recording, so
it proved nothing about the app. Re-run from the installed, signed bundle
through LaunchServices — `open -a "Open Dictate" --args --tap-check`, which
writes its report to the data directory because a LaunchServices start has
nowhere to print — it captured the same way, peak 0.3589.

What `tccd` logged while it ran:

```
09:24:45  kTCCServiceAudioCapture   Sub:{app.opendictate.desktop}  Unknown (None)
09:24:46  AUTHREQ_PROMPTING         service=kTCCServiceAudioCapture
09:24:50  kTCCServiceAudioCapture   Sub:{app.opendictate.desktop}  Allowed (User Consent)
...
09:25:07  kTCCServiceScreenCapture  Sub:{app.opendictate.desktop}  Unknown (None)
```

Three things fall out of that:

- **The subject is the app**, `app.opendictate.desktop` with a
  `responsible_path` inside `/Applications/Open Dictate.app` — not the
  terminal, not whatever launched it. The grant belongs to the bundle and
  lives with its signature, as the microphone and accessibility grants
  already do.
- **The right is `kTCCServiceAudioCapture`** — System Audio Recording — and
  the app is prompted for it on first use: unknown, prompt, allowed, in that
  order, at the first tap rather than at launch. So meeting mode has to
  survive a first run whose opening seconds are silent behind a dialog, and
  must not claim to be recording everyone until it is.
- **Screen recording is not required.** `kTCCServiceScreenCapture` stayed
  `Unknown (None)` for the app throughout, and the audio arrived anyway.
  This was the thing most worth being true: a call can be recorded without
  the app ever asking to see the screen.

`NSAudioCaptureUsageDescription` is in both plists and the prompt carries it.

### What the spike did not settle

- ~~**Permission.**~~ Settled, see above.
- **The microphone in the same device.** The spike's aggregate contains only
  the tap. Meetings need the mic in it too, through
  `kAudioAggregateDeviceSubDeviceListKey`, so both sides arrive mixed on one
  stream.
- **Format conversion.** The tap gives 48 kHz stereo float; whisper.cpp wants
  16 kHz mono. Downmix and resample on the way to disk.
- **The OS floor** is still macOS 14.2-14.4 against this bundle's 11.0, so
  the tap path has to be resolved at runtime with a mic-only fallback.
- **Output changes mid-meeting** — headphones unplugged, AirPods connecting.
  The aggregate is built around one output UID; it likely needs rebuilding
  on a device-change notification without dropping the recording.

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
