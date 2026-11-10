//! Microphone capture with cpal, plus WAV writing for the speech models.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{Arc, Mutex};

pub struct Recording {
    stream: cpal::Stream,
    samples: Arc<Mutex<Vec<f32>>>,
    level: Arc<Mutex<f32>>,
    rate: u32,
    started: std::time::Instant,
    /// Which device and format is being recorded, for diagnostics.
    info: String,
    /// Last error the audio system reported while recording.
    error: Arc<Mutex<Option<String>>>,
}

fn device_name(d: &cpal::Device) -> Option<String> {
    d.description().ok().map(|desc| desc.name().to_string())
}

/// Names of every microphone the system can record from.
pub fn input_devices() -> Vec<String> {
    let mut names: Vec<String> = cpal::default_host()
        .input_devices()
        .map(|it| it.filter_map(|d| device_name(&d)).collect())
        .unwrap_or_default();
    names.dedup();
    names
}

/// Name of the microphone the system currently uses by default.
pub fn default_input_name() -> Option<String> {
    cpal::default_host().default_input_device().and_then(|d| device_name(&d))
}

pub(crate) fn find_input(mic: &str) -> Result<cpal::Device, String> {
    let host = cpal::default_host();
    let chosen = if mic.trim().is_empty() {
        None
    } else {
        host.input_devices()
            .ok()
            .and_then(|mut it| it.find(|d| device_name(d).as_deref() == Some(mic)))
    };
    chosen
        .or_else(|| host.default_input_device())
        .ok_or_else(|| "No microphone found. Check System Settings › Sound › Input.".to_string())
}

pub(crate) fn input_channels(device: &cpal::Device) -> Result<usize, String> {
    device
        .default_input_config()
        .map(|c| c.channels().max(1) as usize)
        .map_err(|e| format!("Microphone unavailable: {e}"))
}

pub(crate) fn device_label(device: &cpal::Device) -> String {
    device_name(device).unwrap_or_else(|| "unknown".into())
}

pub(crate) fn input_stream(
    device: &cpal::Device,
    on_data: impl FnMut(&[f32], u64) + Send + 'static,
    mut on_error: impl FnMut(String) + Send + 'static,
) -> Result<(cpal::Stream, u32, usize), String> {
    let supported = device
        .default_input_config()
        .map_err(|e| format!("Microphone unavailable: {e}"))?;
    let format = supported.sample_format();
    let config = supported.config();
    let channels = config.channels.max(1) as usize;
    let rate = config.sample_rate;
    let err = move |e: cpal::Error| {
        eprintln!("audio stream error: {e}");
        if stops_the_stream(e.kind()) {
            on_error(e.to_string());
        }
    };
    fn build<T: cpal::SizedSample + 'static>(
        device: &cpal::Device,
        config: cpal::StreamConfig,
        convert: fn(T) -> f32,
        mut on_data: impl FnMut(&[f32], u64) + Send + 'static,
        err: impl FnMut(cpal::Error) + Send + 'static,
    ) -> Result<cpal::Stream, cpal::Error> {
        let mut buf = Vec::<f32>::new();
        device.build_input_stream::<T, _, _>(
            config,
            move |data, info| {
                buf.clear();
                buf.extend(data.iter().map(|&v| convert(v)));
                on_data(&buf, info.timestamp().capture.as_nanos() as u64);
            },
            err,
            None,
        )
    }
    let stream = match format {
        cpal::SampleFormat::F32 => build::<f32>(device, config, |v| v, on_data, err),
        cpal::SampleFormat::I16 => build::<i16>(device, config, |v| v as f32 / 32768.0, on_data, err),
        cpal::SampleFormat::U16 => {
            build::<u16>(device, config, |v| (v as f32 - 32768.0) / 32768.0, on_data, err)
        }
        cpal::SampleFormat::I32 => build::<i32>(device, config, |v| v as f32 / 2147483648.0, on_data, err),
        other => return Err(format!("Unsupported microphone format {other:?}")),
    }
    .map_err(|e| format!("Could not open the microphone: {e}"))?;
    stream.play().map_err(|e| format!("Could not start the microphone: {e}"))?;
    Ok((stream, rate, channels))
}

fn push(frames: &[f32], gain: f32, channels: usize, out: &Mutex<Vec<f32>>, level: &Mutex<f32>) {
    let mut peak = 0.0f32;
    let mut buf = out.lock().unwrap();
    for v in crate::mic::pick_channels(frames, channels) {
        let v = (v * gain).clamp(-1.0, 1.0);
        peak = peak.max(v.abs());
        buf.push(v);
    }
    let mut l = level.lock().unwrap();
    *l = (*l * 0.6).max(peak);
}

impl Recording {
    /// `mic` is a microphone name ("" = system default); `gain` boosts quiet microphones.
    pub fn start(mic: &str, gain: f32) -> Result<Self, String> {
        let device = find_input(mic)?;
        let gain = gain.clamp(0.1, 8.0);
        let supported = device
            .default_input_config()
            .map_err(|e| format!("Microphone unavailable: {e}"))?;
        let (format, expected) = (supported.sample_format(), supported.channels().max(1) as usize);

        let samples = Arc::new(Mutex::new(Vec::<f32>::with_capacity(supported.sample_rate() as usize * 30)));
        let level = Arc::new(Mutex::new(0.0f32));

        let error = Arc::new(Mutex::new(None::<String>));
        let er = error.clone();
        let (s, l) = (samples.clone(), level.clone());
        let (stream, rate, channels) = input_stream(
            &device,
            move |data, _| push(data, gain, expected, &s, &l),
            move |e| *er.lock().unwrap() = Some(e),
        )?;
        if channels != expected {
            return Err("The microphone changed format while opening. Try again.".into());
        }
        let info = format!("{} · {format:?} · {rate} Hz · {channels} ch", device_label(&device));

        Ok(Self { stream, samples, level, rate, started: std::time::Instant::now(), info, error })
    }

    pub fn info(&self) -> String {
        self.info.clone()
    }

    pub fn error(&self) -> Option<String> {
        self.error.lock().unwrap().clone()
    }

    /// How many samples have arrived so far (0 means the system sends nothing).
    pub fn sample_count(&self) -> usize {
        self.samples.lock().unwrap().len()
    }

    /// Loudest sample so far.
    pub fn peak(&self) -> f32 {
        self.samples.lock().unwrap().iter().fold(0.0f32, |m, v| m.max(v.abs()))
    }

    pub fn level(&self) -> f32 {
        *self.level.lock().unwrap()
    }

    pub fn elapsed(&self) -> f32 {
        self.started.elapsed().as_secs_f32()
    }

    /// Stops the microphone and returns 16 kHz mono samples.
    pub fn finish(self) -> Clip {
        drop(self.stream);
        let raw = std::mem::take(&mut *self.samples.lock().unwrap());
        Clip { samples: resample(&raw, self.rate, 16_000) }
    }
}

pub struct Clip {
    pub samples: Vec<f32>,
}

impl Clip {
    pub fn seconds(&self) -> f32 {
        self.samples.len() as f32 / 16_000.0
    }

    /// True when the clip is too short or too quiet to hold speech.
    pub fn is_silent(&self) -> bool {
        if self.seconds() < 0.3 {
            return true;
        }
        let peak = self.samples.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        peak < 0.01
    }

    /// 16-bit PCM mono WAV at 16 kHz, the format whisper.cpp expects.
    pub fn to_wav(&self) -> Vec<u8> {
        let data_len = (self.samples.len() * 2) as u32;
        let mut out = Vec::with_capacity(44 + data_len as usize);
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&(36 + data_len).to_le_bytes());
        out.extend_from_slice(b"WAVEfmt ");
        out.extend_from_slice(&16u32.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes()); // PCM
        out.extend_from_slice(&1u16.to_le_bytes()); // mono
        out.extend_from_slice(&16_000u32.to_le_bytes());
        out.extend_from_slice(&32_000u32.to_le_bytes()); // byte rate
        out.extend_from_slice(&2u16.to_le_bytes()); // block align
        out.extend_from_slice(&16u16.to_le_bytes()); // bits
        out.extend_from_slice(b"data");
        out.extend_from_slice(&data_len.to_le_bytes());
        for s in &self.samples {
            let v = (s.clamp(-1.0, 1.0) * 32767.0) as i16;
            out.extend_from_slice(&v.to_le_bytes());
        }
        out
    }
}

fn stops_the_stream(kind: cpal::ErrorKind) -> bool {
    !matches!(kind, cpal::ErrorKind::Xrun | cpal::ErrorKind::DeviceChanged | cpal::ErrorKind::RealtimeDenied)
}

fn resample(input: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == to || input.is_empty() {
        return input.to_vec();
    }
    let ratio = from as f64 / to as f64;
    let len = (input.len() as f64 / ratio) as usize;
    (0..len)
        .map(|i| {
            // Average the source samples that fall into this output slot (cheap low-pass).
            let start = i as f64 * ratio;
            let end = ((i + 1) as f64 * ratio).min(input.len() as f64);
            let (a, b) = (start as usize, (end as usize).max(start as usize + 1).min(input.len()));
            let slice = &input[a..b];
            slice.iter().sum::<f32>() / slice.len() as f32
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{push, stops_the_stream};
    use std::sync::Mutex;
    use cpal::ErrorKind;

    #[test]
    fn overloads_and_reroutes_do_not_count_as_the_microphone_stopping() {
        for kind in [ErrorKind::Xrun, ErrorKind::DeviceChanged, ErrorKind::RealtimeDenied] {
            assert!(!stops_the_stream(kind), "{kind:?}");
        }
    }

    #[test]
    fn a_disconnect_or_a_sample_rate_change_counts_as_the_microphone_stopping() {
        for kind in [ErrorKind::DeviceNotAvailable, ErrorKind::StreamInvalidated, ErrorKind::BackendError] {
            assert!(stops_the_stream(kind), "{kind:?}");
        }
    }

    #[test]
    fn dictation_on_a_16_channel_interface_keeps_the_mic_at_full_level() {
        let mut data = vec![0.0f32; 16 * 4];
        for frame in data.chunks_exact_mut(16) {
            frame[0] = 0.3;
        }
        let (out, level) = (Mutex::new(Vec::new()), Mutex::new(0.0f32));
        push(&data, 2.0, 16, &out, &level);
        let peak = out.lock().unwrap().iter().fold(0.0f32, |p, v| p.max(v.abs()));
        assert!((peak - 0.6).abs() < 1e-6, "{peak}");
        assert!((*level.lock().unwrap() - 0.6).abs() < 1e-6);
    }
}
