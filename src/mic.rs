//! The meeting microphone as its own cpal stream, outside the system-audio aggregate.

use crate::audio;
use crate::tap::{mix_mic, MIC_INPUTS};
use cpal::traits::StreamTrait;
use std::sync::{Arc, Mutex};

pub(crate) fn pick_channels(interleaved: &[f32], channels: usize) -> Vec<f32> {
    let channels = channels.max(1);
    let frames = interleaved.len() / channels;
    let picked = (0..channels.min(MIC_INPUTS))
        .map(|c| interleaved.iter().skip(c).step_by(channels).take(frames).copied().collect())
        .collect();
    mix_mic(picked, frames)
}

#[derive(Default)]
struct Captured {
    samples: Vec<f32>,
    first_ns: Option<u64>,
}

pub struct MicStream {
    stream: cpal::Stream,
    captured: Arc<Mutex<Captured>>,
    error: Arc<Mutex<Option<String>>>,
    rate: u32,
    info: String,
}

impl MicStream {
    /// `name` is a microphone name ("" = system default). Never applies input gain.
    pub fn start(name: &str) -> Result<Self, String> {
        let device = audio::find_input(name)?;
        let expected = audio::input_channels(&device)?;
        let captured = Arc::new(Mutex::new(Captured::default()));
        let error = Arc::new(Mutex::new(None::<String>));
        let (c, er) = (captured.clone(), error.clone());
        let (stream, rate, channels) = audio::input_stream(
            &device,
            move |data, host_ns| {
                let mono = pick_channels(data, expected);
                let mut c = c.lock().unwrap();
                if c.samples.is_empty() {
                    c.first_ns = Some(host_ns);
                }
                c.samples.extend_from_slice(&mono);
            },
            move |e| *er.lock().unwrap() = Some(e),
        )?;
        if channels != expected {
            return Err("The microphone changed format while opening. Try again.".into());
        }
        let used = if channels == 1 { "ch 1".to_string() } else { format!("louder of ch 1–{}", channels.min(MIC_INPUTS)) };
        let info = format!("{} · {rate} Hz · {channels} ch · using {used}", audio::device_label(&device));
        Ok(Self { stream, captured, error, rate, info })
    }

    /// Samples at the device rate since the last drain, and the host ns of the first of them.
    pub fn drain(&self) -> (Vec<f32>, Option<u64>) {
        let mut c = self.captured.lock().unwrap();
        (std::mem::take(&mut c.samples), c.first_ns.take())
    }

    pub fn rate(&self) -> f64 {
        self.rate as f64
    }

    pub fn info(&self) -> String {
        self.info.clone()
    }

    pub fn pause(&self) {
        if let Err(e) = self.stream.pause() {
            eprintln!("could not pause the meeting mic: {e}");
        }
    }

    pub fn failed(&self) -> Option<String> {
        self.error.lock().unwrap().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::pick_channels;
    use crate::tap::mix_mic;

    fn peak(v: &[f32]) -> f32 {
        v.iter().fold(0f32, |m, s| m.max(s.abs()))
    }

    fn quiet() -> Vec<f32> {
        (0..480).map(|i| if i % 2 == 0 { 0.001 } else { -0.001 }).collect()
    }

    fn tone(level: f32) -> Vec<f32> {
        (0..480).map(|i| level * (i as f32 * 0.1).sin()).collect()
    }

    fn interleave(channels: &[Vec<f32>]) -> Vec<f32> {
        (0..channels[0].len()).flat_map(|f| channels.iter().map(move |c| c[f])).collect()
    }

    #[test]
    fn a_mic_on_one_of_sixteen_interface_channels_reaches_whisper_at_full_level() {
        let mut channels: Vec<Vec<f32>> = (0..16).map(|_| quiet()).collect();
        let mut loud = quiet();
        loud[10] = 0.3;
        channels[1] = loud;
        let out = mix_mic(channels, 480);
        assert!((peak(&out) - 0.3).abs() < 1e-6, "peak was {}", peak(&out));
    }

    #[test]
    fn loopback_channels_after_the_first_two_are_never_taken_as_the_mic() {
        let mut channels: Vec<Vec<f32>> = (0..16).map(|_| quiet()).collect();
        channels[0] = tone(0.05);
        for c in [2, 3, 6, 7] {
            channels[c] = tone(0.47);
        }
        assert_eq!(mix_mic(channels.clone(), 480), channels[0]);
    }

    #[test]
    fn a_mono_mic_passes_through_unchanged() {
        let mic = tone(0.2);
        assert_eq!(mix_mic(vec![mic.clone()], 480), mic);
    }

    #[test]
    fn the_louder_of_two_loud_channels_is_taken() {
        let channels = vec![tone(0.2), tone(0.5)];
        assert_eq!(mix_mic(channels.clone(), 480), channels[1]);
    }

    #[test]
    fn no_mic_channels_gives_silence_of_the_frame_length() {
        assert_eq!(mix_mic(vec![], 4), vec![0f32; 4]);
    }

    #[test]
    fn a_voice_on_channel_0_of_16_interleaved_reaches_full_level() {
        let mut channels: Vec<Vec<f32>> = (0..16).map(|_| quiet()).collect();
        channels[0] = tone(0.3);
        for c in [2, 3, 6, 7] {
            channels[c] = tone(0.47);
        }
        assert_eq!(pick_channels(&interleave(&channels), 16), channels[0]);
    }

    #[test]
    fn a_stereo_mic_takes_the_louder_channel() {
        let channels = vec![tone(0.1), tone(0.4)];
        assert_eq!(pick_channels(&interleave(&channels), 2), channels[1]);
    }

    #[test]
    fn a_mono_mic_passes_through() {
        let mic = tone(0.2);
        assert_eq!(pick_channels(&mic, 1), mic);
    }
}
