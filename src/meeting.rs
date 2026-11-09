//! Meeting mode: a long recording of both sides of a call, transcribed as it
//! runs and kept on disk.
//!
//! A meeting is too long to hold in memory and too long to transcribe in one
//! go, so it is cut into chunks that are written, transcribed and thrown away
//! while the next one fills. That keeps memory flat however long the meeting
//! runs, puts a transcript on screen before it ends, and costs at most one
//! chunk if the app dies.
//!
//! ```text
//! meetings/<started>/
//!   meta.json         title, started, ended, source
//!   transcript.jsonl  one line per chunk: { at_ms, secs, text }
//!   audio/000.wav     deleted as each one is transcribed
//! ```
//!
//! JSONL because a meeting in progress is appended to every half minute: a
//! crash truncates one line instead of corrupting the file.

use crate::settings::{self, Settings};
use crate::tap::SystemCapture;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// How much audio goes into one chunk. Long enough that whisper has context
/// to work with, short enough that the transcript keeps up with the room.
const CHUNK_SECS: f32 = 30.0;
/// What whisper.cpp wants, and what the chunks are written as.
const OUT_RATE: u32 = 16_000;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Meta {
    pub id: String,
    pub title: String,
    pub started: u64,
    pub ended: Option<u64>,
    /// "You and the room" or "Everyone on the call", for the UI to show.
    pub source: String,
    /// Loudest sample in the whole meeting, 0 to 1. The only way to tell a
    /// quiet room from a recorder that heard nothing.
    #[serde(default)]
    pub peak: f32,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Segment {
    pub at_ms: u64,
    pub secs: f32,
    pub text: String,
}

/// What the UI reads while a meeting is running.
#[derive(Default)]
pub struct Live {
    pub elapsed: f32,
    pub segments: Vec<Segment>,
    /// Set when a chunk failed to transcribe; the recording carries on.
    pub warning: Option<String>,
    /// Loudest sample seen since the meeting started. Zero after the first
    /// chunk means nothing is reaching the recorder at all, which is a very
    /// different problem from a quiet room and has to be said differently.
    pub peak: f32,
    pub chunks: usize,
}

pub fn root() -> PathBuf {
    settings::data_dir().join("meetings")
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Every meeting on disk, newest first.
pub fn list() -> Vec<Meta> {
    let mut out: Vec<Meta> = std::fs::read_dir(root())
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| std::fs::read_to_string(e.path().join("meta.json")).ok())
        .filter_map(|s| serde_json::from_str::<Meta>(&s).ok())
        .collect();
    out.sort_by(|a, b| b.started.cmp(&a.started));
    out
}

pub fn transcript(id: &str) -> Vec<Segment> {
    std::fs::read_to_string(root().join(id).join("transcript.jsonl"))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str::<Segment>(l).ok())
        .collect()
}

pub fn rename(id: &str, title: &str) {
    let path = root().join(id).join("meta.json");
    if let Some(mut m) = std::fs::read_to_string(&path).ok().and_then(|s| serde_json::from_str::<Meta>(&s).ok()) {
        m.title = title.to_string();
        let _ = std::fs::write(&path, serde_json::to_string_pretty(&m).unwrap_or_default());
    }
}

pub fn delete(id: &str) {
    // Guard against a crafted id walking out of the meetings directory.
    if id.contains('/') || id.contains("..") {
        return;
    }
    let _ = std::fs::remove_dir_all(root().join(id));
}

/// Average every `ratio` samples. The tap runs at 48k and whisper wants 16k,
/// so this is a box filter of three - crude as a low-pass, but speech has
/// nothing up there worth protecting.
fn to_16k(input: &[f32], from: f64) -> Vec<f32> {
    if input.is_empty() {
        return Vec::new();
    }
    let ratio = (from / OUT_RATE as f64).max(1.0);
    let out_len = (input.len() as f64 / ratio) as usize;
    (0..out_len)
        .map(|i| {
            let start = (i as f64 * ratio) as usize;
            let end = (((i + 1) as f64 * ratio) as usize).min(input.len()).max(start + 1);
            let slice = &input[start..end.min(input.len())];
            slice.iter().sum::<f32>() / slice.len().max(1) as f32
        })
        .collect()
}

fn wav_16k_mono(samples: &[f32]) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&OUT_RATE.to_le_bytes());
    out.extend_from_slice(&(OUT_RATE * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        out.extend_from_slice(&((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
    }
    out
}

/// A line per chunk, beside the transcript. A meeting that comes back empty
/// has to be able to say why without being run again under a debugger.
fn log(dir: &Path, line: &str) {
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(dir.join("levels.log")) {
        let _ = writeln!(f, "{line}");
    }
}

fn append_segment(dir: &Path, seg: &Segment) {
    use std::io::Write;
    if let Ok(line) = serde_json::to_string(seg) {
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(dir.join("transcript.jsonl")) {
            let _ = writeln!(f, "{line}");
        }
    }
}

/// A meeting being recorded. Dropping it stops the recording.
pub struct Session {
    pub id: String,
    pub dir: PathBuf,
    pub source: String,
    pub live: Arc<Mutex<Live>>,
    stop: Arc<AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
}

impl Session {
    pub fn start(s: &Settings) -> Result<Self, String> {
        let cap = SystemCapture::start(crate::tap::default_input())?;
        let source = if cap.has_mic { "You and everyone on the call" } else { "Everyone on the call" };
        let started = now();
        let id = chrono::Local::now().format("%Y-%m-%d-%H%M%S").to_string();
        let dir = root().join(&id);
        std::fs::create_dir_all(dir.join("audio")).map_err(|e| format!("could not make room for the meeting: {e}"))?;
        let meta = Meta {
            id: id.clone(),
            title: chrono::Local::now().format("%A %H:%M").to_string(),
            started,
            ended: None,
            source: source.to_string(),
            peak: 0.0,
        };
        let _ = std::fs::write(dir.join("meta.json"), serde_json::to_string_pretty(&meta).unwrap_or_default());

        let live = Arc::new(Mutex::new(Live::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let worker = std::thread::spawn({
            let (dir, live, stop) = (dir.clone(), live.clone(), stop.clone());
            let settings = s.clone();
            move || run(cap, dir, live, stop, settings)
        });
        Ok(Self { id, dir, source: source.to_string(), live, stop, worker: Some(worker) })
    }

    /// Stops recording and waits for the last chunk to be transcribed.
    pub fn stop(mut self) -> String {
        self.finish();
        self.id.clone()
    }

    fn finish(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(w) = self.worker.take() {
            let _ = w.join();
        }
        let path = self.dir.join("meta.json");
        if let Some(mut m) = std::fs::read_to_string(&path).ok().and_then(|s| serde_json::from_str::<Meta>(&s).ok()) {
            m.ended = Some(now());
            m.peak = self.live.lock().map(|l| l.peak).unwrap_or(0.0);
            let _ = std::fs::write(&path, serde_json::to_string_pretty(&m).unwrap_or_default());
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if self.worker.is_some() {
            self.finish();
        }
    }
}

/// Drain, chunk, write, transcribe, repeat. One worker: transcription is the
/// slow part and running several at once only starves the machine the meeting
/// is happening on.
fn run(cap: SystemCapture, dir: PathBuf, live: Arc<Mutex<Live>>, stop: Arc<AtomicBool>, s: Settings) {
    let rate = cap.rate;
    let per_chunk = (rate * CHUNK_SECS as f64) as usize;
    let mut buf: Vec<f32> = Vec::with_capacity(per_chunk);
    let mut index = 0usize;
    let mut at_ms = 0u64;
    let started = std::time::Instant::now();

    loop {
        let ending = stop.load(Ordering::Relaxed);
        buf.extend(cap.drain());
        if let Ok(mut l) = live.lock() {
            l.elapsed = started.elapsed().as_secs_f32();
        }

        // Ending means ending: stop the device before collecting the rest,
        // or every pass picks up another quarter second and writes another
        // chunk for it - 133 of them, in the first run of this.
        if ending {
            cap.pause();
            buf.extend(cap.drain());
        }
        let full = buf.len() >= per_chunk;
        if full || (ending && !buf.is_empty()) {
            let take = if full { per_chunk } else { buf.len() };
            let chunk: Vec<f32> = buf.drain(..take).collect();
            let secs = chunk.len() as f32 / rate as f32;
            let samples = to_16k(&chunk, rate);
            let path = dir.join("audio").join(format!("{index:03}.wav"));
            let wav = wav_16k_mono(&samples);
            let _ = std::fs::write(&path, &wav);

            // Whisper invents speech out of near-silence: the quiet tail of
            // one test came back as "Akuipito. Akuipito. Akuipito.". A
            // meeting is mostly silence, so this gate matters more here than
            // it does for a dictation someone is holding a key down for.
            let rms = (samples.iter().map(|x| x * x).sum::<f32>() / samples.len().max(1) as f32).sqrt();
            let peak = samples.iter().fold(0f32, |m, x| m.max(x.abs()));
            // The gate was first set from one loud test - speech at peak
            // 0.52 against a silent tail at 0.036 - and a real call went
            // straight through it: two meetings, no transcript, no clue why.
            // It sits just above digital silence now, and every chunk's
            // level is written to levels.log so the next surprise is
            // readable instead of guessed at.
            log(&dir, &format!("{index:03} {secs:5.1}s peak {peak:.4} rms {rms:.5}"));
            if let Ok(mut l) = live.lock() {
                l.peak = l.peak.max(peak);
                l.chunks += 1;
            }
            // Two conditions, because one threshold could not separate the
            // cases seen so far: speech at peak 0.43, a noisy tail at 0.036
            // that hallucinated "Akuipito", a quieter one at 0.021 that
            // hallucinated a whole sentence. Anything below 0.03 is the noise
            // floor, and a short tail has to be obviously speech to count -
            // that is where the inventions live.
            if peak < 0.03 || rms < 0.002 || (secs < 5.0 && peak < 0.10) {
                log(&dir, "    skipped: too quiet to be speech");
                let _ = std::fs::remove_file(&path);
                at_ms += (secs * 1000.0) as u64;
                index += 1;
                if ending && buf.is_empty() {
                    break;
                }
                continue;
            }

            match crate::engines::transcribe(&s, &wav) {
                Ok(text) if !text.trim().is_empty() => {
                    log(&dir, &format!("    transcribed: {}", text.trim().chars().take(60).collect::<String>()));
                    let seg = Segment { at_ms, secs, text: text.trim().to_string() };
                    append_segment(&dir, &seg);
                    if let Ok(mut l) = live.lock() {
                        l.segments.push(seg);
                    }
                }
                Ok(text) => log(&dir, &format!("    transcribed to nothing ({} chars raw)", text.len())),
                Err(e) => {
                    log(&dir, &format!("    failed: {e}"));
                    if let Ok(mut l) = live.lock() {
                        l.warning = Some(e);
                    }
                }
            }
            // The transcript is the thing worth keeping; the audio is 1.9 MB
            // a minute and a week of meetings would fill a disk.
            let _ = std::fs::remove_file(&path);
            at_ms += (secs * 1000.0) as u64;
            index += 1;
            continue;
        }
        if ending {
            if let Ok(mut l) = live.lock() {
                if l.chunks == 0 {
                    l.warning = Some("No audio reached the recorder.".into());
                    log(&dir, "no audio arrived at all");
                }
            }
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
}
