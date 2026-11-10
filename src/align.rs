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

pub const HOLD_MS: u64 = 500;
pub const DRIFT_START_MS: f32 = 10.0;
pub const DRIFT_STOP_MS: f32 = 2.0;
pub const MAX_FIX_PER_TAKE: usize = 4;

#[derive(Default)]
struct Side {
    buf: Vec<f32>,
    count: u64,
    first: Option<(u64, u64)>,
    last: Option<(u64, u64)>,
    skip: usize,
    fresh: bool,
    heard_at: Option<u64>,
    padding: bool,
}

impl Side {
    fn push(&mut self, samples: &[f32], first_host_ns: Option<u64>) {
        if let Some(t) = first_host_ns {
            let mark = Some((t, self.count));
            if self.first.is_none() {
                self.first = mark;
            } else {
                self.last = mark;
            }
        }
        self.buf.extend_from_slice(samples);
        self.count += samples.len() as u64;
        self.fresh = true;
    }

    fn span(&self) -> Option<(f64, f64)> {
        let ((t0, n0), (t1, n1)) = (self.first?, self.last?);
        (t1 > t0).then(|| ((t1 - t0) as f64, (n1 - n0) as f64))
    }

    fn pad_after_hold(&mut self, other_len: usize, now_ns: u64, name: &'static str, events: &mut Vec<&'static str>) {
        let silent_ns = now_ns.saturating_sub(self.heard_at.unwrap_or(now_ns));
        if silent_ns >= HOLD_MS * 1_000_000 && self.buf.len() < other_len {
            if !self.padding {
                events.push(name);
                self.padding = true;
            }
            self.buf.resize(other_len, 0.0);
        }
    }
}

pub struct Aligner {
    rate: f64,
    mic: Side,
    call: Side,
    trimmed: bool,
    fixed: f64,
    correcting: bool,
    events: Vec<&'static str>,
}

impl Aligner {
    pub fn new(rate: f64) -> Self {
        Aligner {
            rate,
            mic: Side::default(),
            call: Side::default(),
            trimmed: false,
            fixed: 0.0,
            correcting: false,
            events: Vec::new(),
        }
    }

    pub fn push_call(&mut self, samples: &[f32], first_host_ns: Option<u64>) {
        self.call.push(samples, first_host_ns);
    }

    pub fn push_mic(&mut self, samples: &[f32], first_host_ns: Option<u64>) {
        self.mic.push(samples, first_host_ns);
    }

    fn offset_samples(&self) -> f64 {
        match (self.mic.span(), self.call.span()) {
            (Some((mic_ns, mic_n)), Some((call_ns, call_n))) => mic_n - mic_ns * call_n / call_ns - self.fixed,
            _ => 0.0,
        }
    }

    pub fn offset_ms(&self) -> f32 {
        (self.offset_samples() / self.rate * 1000.0) as f32
    }

    pub fn padded(&mut self) -> Option<&'static str> {
        (!self.events.is_empty()).then(|| self.events.remove(0))
    }

    pub fn take(&mut self, now_ns: u64) -> (Vec<f32>, Vec<f32>) {
        for side in [&mut self.mic, &mut self.call] {
            if std::mem::take(&mut side.fresh) {
                side.heard_at = Some(now_ns);
                side.padding = false;
            }
            side.heard_at.get_or_insert(now_ns);
        }
        if !self.trimmed
            && let (Some((mic_t, _)), Some((call_t, _))) = (self.mic.first, self.call.first)
        {
            let lead = (mic_t.abs_diff(call_t) as f64 * self.rate / 1e9).round() as usize;
            if mic_t < call_t { &mut self.mic } else { &mut self.call }.skip = lead;
            self.trimmed = true;
        }
        for side in [&mut self.mic, &mut self.call] {
            let k = side.skip.min(side.buf.len());
            side.buf.drain(..k);
            side.skip -= k;
        }
        self.correct_drift();
        let (mic_len, call_len) = (self.mic.buf.len(), self.call.buf.len());
        self.mic.pad_after_hold(call_len, now_ns, "mic", &mut self.events);
        self.call.pad_after_hold(mic_len, now_ns, "call", &mut self.events);
        let n = self.mic.buf.len().min(self.call.buf.len());
        (self.mic.buf.drain(..n).collect(), self.call.buf.drain(..n).collect())
    }

    fn correct_drift(&mut self) {
        let ms_to_samples = self.rate / 1000.0;
        let offset = self.offset_samples();
        let stop = DRIFT_STOP_MS as f64 * ms_to_samples;
        if offset.abs() > DRIFT_START_MS as f64 * ms_to_samples {
            self.correcting = true;
        }
        if !self.correcting {
            return;
        }
        let window = self.mic.buf.len().min(self.call.buf.len());
        let needed = (offset.abs() - stop).floor().max(0.0) as usize + 1;
        // Spacing of at least 3 keeps two corrections from ever touching in the output.
        let count = MAX_FIX_PER_TAKE.min(needed).min((window / 3).saturating_sub(1));
        for j in (0..count).rev() {
            let at = (j + 1) * window / (count + 1);
            if offset > 0.0 {
                self.mic.buf.remove(at);
            } else {
                self.mic.buf.insert(at, self.mic.buf[at]);
            }
        }
        self.fixed += count as f64 * offset.signum();
        if self.offset_samples().abs() < stop {
            self.correcting = false;
        }
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

#[cfg(test)]
mod aligner_tests {
    use super::*;

    const RATE: f64 = 48_000.0;
    const DRAIN: usize = 12_000;
    const DRAIN_NS: u64 = 250_000_000;

    fn ramp(from: usize, len: usize) -> Vec<f32> {
        (from..from + len).map(|i| i as f32).collect()
    }

    fn run_drift(ppm: f64, takes: usize, mut each: impl FnMut(&Aligner, &[f32])) -> Aligner {
        let mut al = Aligner::new(RATE);
        let mut mic_sent = 0;
        for k in 0..takes {
            let t = 1_000_000_000 + k as u64 * DRAIN_NS;
            let mic_due = ((k + 1) as f64 * DRAIN as f64 * (1.0 + ppm * 1e-6)).round() as usize;
            al.push_call(&vec![0.0; DRAIN], Some(t));
            al.push_mic(&ramp(mic_sent, mic_due - mic_sent), Some(t));
            mic_sent = mic_due;
            let (mine, theirs) = al.take(t + DRAIN_NS);
            assert_eq!(mine.len(), theirs.len());
            each(&al, &mine);
        }
        al
    }

    #[test]
    fn the_stream_that_started_earlier_is_trimmed_so_both_begin_together() {
        let mut al = Aligner::new(RATE);
        let call = ramp(0, 24_000);
        al.push_call(&call, Some(1_000_000_000));
        al.push_mic(&vec![0.5; 24_000], Some(1_020_000_000));
        let (mine, theirs) = al.take(1_500_000_000);
        assert_eq!(mine.len(), theirs.len());
        assert_eq!(theirs[0], call[960]);
    }

    #[test]
    fn unequal_arrivals_are_held_not_dropped() {
        let mut al = Aligner::new(RATE);
        let call = ramp(0, 24_000);
        let mic: Vec<f32> = ramp(100_000, 24_000);
        al.push_call(&call[..14_400], None);
        al.push_mic(&mic[..4_800], None);
        let (mut mine, mut theirs) = al.take(0);
        assert_eq!((mine.len(), theirs.len()), (4_800, 4_800));
        al.push_call(&call[14_400..], None);
        al.push_mic(&mic[4_800..14_400], None);
        let (m, t) = al.take(100_000_000);
        assert_eq!((m.len(), t.len()), (9_600, 9_600));
        mine.extend(m);
        theirs.extend(t);
        assert_eq!(mine, mic[..14_400]);
        assert_eq!(theirs, call[..14_400]);
    }

    #[test]
    fn a_mic_running_100ppm_fast_stays_within_10ms_over_an_hour() {
        run_drift(100.0, 14_400, |al, _| {
            assert!(al.offset_ms().abs() <= DRIFT_START_MS, "offset {}", al.offset_ms());
        });
    }

    #[test]
    fn drift_moves_at_most_four_spaced_mic_samples_per_take() {
        for (ppm, changed_step) in [(10_000.0, 2.0), (-10_000.0, 0.0)] {
            let mut prev: Option<f32> = None;
            let mut corrected_takes = 0;
            run_drift(ppm, 40, |_, mine| {
                let mut seq: Vec<f32> = prev.into_iter().collect();
                seq.extend_from_slice(mine);
                let changed: Vec<usize> = seq
                    .windows(2)
                    .enumerate()
                    .filter(|(_, w)| w[1] - w[0] != 1.0)
                    .map(|(i, w)| {
                        assert_eq!(w[1] - w[0], changed_step, "only single drops or repeats");
                        i
                    })
                    .collect();
                assert!(changed.len() <= MAX_FIX_PER_TAKE, "{} changes", changed.len());
                assert!(changed.windows(2).all(|p| p[1] - p[0] > 1), "adjacent changes {changed:?}");
                corrected_takes += !changed.is_empty() as usize;
                prev = mine.last().copied().or(prev);
            });
            assert!(corrected_takes > 0, "{ppm} ppm never corrected");
        }
    }

    fn assert_dead_side_is_padded(mic_dies: bool) {
        let mut al = Aligner::new(RATE);
        let mut live_sent = Vec::new();
        let mut live_got = Vec::new();
        let mut dead_got_after_stop = Vec::new();
        let mut first_padded_take = None;
        let mut padded = Vec::new();
        for k in 0..12u64 {
            let now = k * DRAIN_NS;
            let chunk = ramp(k as usize * DRAIN, DRAIN);
            live_sent.extend_from_slice(&chunk);
            let dead_chunk = (k < 4).then(|| ramp(1_000_000 + k as usize * DRAIN, DRAIN));
            if mic_dies {
                al.push_call(&chunk, None);
                if let Some(d) = &dead_chunk { al.push_mic(d, None) }
            } else {
                al.push_mic(&chunk, None);
                if let Some(d) = &dead_chunk { al.push_call(d, None) }
            }
            let (mine, theirs) = al.take(now);
            let (live, dead) = if mic_dies { (theirs, mine) } else { (mine, theirs) };
            if k >= 4 {
                if !live.is_empty() && first_padded_take.is_none() {
                    first_padded_take = Some(k);
                }
                dead_got_after_stop.extend(dead);
            }
            live_got.extend(live);
            padded.extend(al.padded());
        }
        let last_arrival = 3 * DRAIN_NS;
        let hold_passes = (last_arrival + HOLD_MS * 1_000_000).div_ceil(DRAIN_NS);
        assert!(first_padded_take.unwrap() <= hold_passes + 2);
        assert_eq!(live_got, live_sent);
        assert!(dead_got_after_stop.iter().all(|s| *s == 0.0));
        assert_eq!(padded, vec![if mic_dies { "mic" } else { "call" }]);
    }

    #[test]
    fn a_dead_mic_is_padded_after_500ms_and_them_never_stalls() {
        assert_dead_side_is_padded(true);
    }

    #[test]
    fn a_stalled_call_is_padded_the_same_way() {
        assert_dead_side_is_padded(false);
    }
}
