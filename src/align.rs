pub struct Resampler {
    step: f64,
    width: usize,
    raw: Vec<f32>,
    filtered: Vec<f32>,
    pos: f64,
}

impl Resampler {
    pub fn new(from: f64, to: f64) -> Self {
        let step = from / to;
        Resampler {
            step,
            width: step.round().max(1.0) as usize,
            raw: Vec::new(),
            filtered: Vec::new(),
            pos: 0.0,
        }
    }

    pub fn push(&mut self, input: &[f32]) -> Vec<f32> {
        if self.step == 1.0 {
            return input.to_vec();
        }
        self.raw.extend_from_slice(input);
        let w = self.width;
        // Averaging before decimating keeps what can't be represented at the lower rate from aliasing.
        self.filtered
            .extend(self.raw.windows(w).map(|win| win.iter().sum::<f32>() / w as f32));
        self.raw.drain(..self.raw.len().saturating_sub(w - 1));

        let mut out = Vec::new();
        while (self.pos as usize) + 1 < self.filtered.len() {
            let i = self.pos as usize;
            let frac = (self.pos - i as f64) as f32;
            out.push(self.filtered[i] + (self.filtered[i + 1] - self.filtered[i]) * frac);
            self.pos += self.step;
        }
        let used = (self.pos as usize).min(self.filtered.len());
        self.filtered.drain(..used);
        self.pos -= used as f64;
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(rate: f64, hz: f64, secs: f64, level: f32) -> Vec<f32> {
        (0..(rate * secs) as usize)
            .map(|i| level * (std::f64::consts::TAU * hz * i as f64 / rate).sin() as f32)
            .collect()
    }

    fn zero_crossings_per_sec(v: &[f32], rate: f64) -> f64 {
        let crossings = v.windows(2).filter(|w| (w[0] < 0.0) != (w[1] < 0.0)).count();
        crossings as f64 / (v.len() as f64 / rate)
    }

    fn peak(v: &[f32]) -> f32 {
        v.iter().fold(0.0, |m, s| m.max(s.abs()))
    }

    fn assert_same_tone(from: f64) {
        let out = Resampler::new(from, 48_000.0).push(&tone(from, 1_000.0, 1.0, 0.3));
        assert!((out.len() as i64 - 48_000).abs() <= 1, "len {}", out.len());
        let crossings = zero_crossings_per_sec(&out, 48_000.0);
        assert!((crossings - 2_000.0).abs() <= 20.0, "crossings/sec {crossings}");
        assert!((peak(&out) - 0.3).abs() <= 0.01, "peak {}", peak(&out));
    }

    #[test]
    fn upsampling_44k1_to_48k_keeps_a_tones_pitch_and_level() {
        assert_same_tone(44_100.0);
    }

    #[test]
    fn downsampling_96k_to_48k_keeps_a_tones_pitch_and_level() {
        assert_same_tone(96_000.0);
    }

    #[test]
    fn chunked_input_resamples_the_same_as_one_push() {
        let input = tone(44_100.0, 1_000.0, 1.0, 0.3);
        let whole = Resampler::new(44_100.0, 48_000.0).push(&input);
        let mut chunked_resampler = Resampler::new(44_100.0, 48_000.0);
        let chunked: Vec<f32> = input.chunks(441).flat_map(|c| chunked_resampler.push(c)).collect();
        assert_eq!(chunked.len(), whole.len());
        for (a, b) in chunked.iter().zip(&whole) {
            assert!((a - b).abs() <= 1e-6, "{a} vs {b}");
        }
    }

    #[test]
    fn equal_rates_pass_through_unchanged() {
        let input = tone(48_000.0, 1_000.0, 0.1, 0.3);
        assert_eq!(Resampler::new(48_000.0, 48_000.0).push(&input), input);
    }
}
