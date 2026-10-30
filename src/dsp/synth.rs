use super::chorus::Chorus;
use super::clipper::soft_clip;
use super::envelope::glide_coef;
use super::limiter::Limiter;
use super::operator::SINE_TABLE;
use super::oversampling::Downsampler2x;
use super::patch::Patch;
use super::polyphony::Polyphony;
use std::sync::LazyLock;

/// Longest stretch rendered in one go. Longer requests are split up.
const MAX_BLOCK: usize = 64;

/// How long the output level takes to follow a gain change (to -60 dB of the
/// difference), so turning the volume doesn't crackle.
const GAIN_GLIDE_TIME: f32 = 0.05;

/// The complete instrument: voices, oversampling, master gain, stereo chorus
/// and protection against overloads. This is the only thing the plugin (and
/// the examples) talk to.
///
/// ```text
///   voices ─▶ downsample ─▶ × volume × gain ─▶ chorus ─▶ limiter ─▶ soft clip ─▶ out
///   └─ twice the rate ─┘                               (stereo from here on)
/// ```
///
/// The limiter turns loud moments down cleanly; the soft clipper after it is
/// only a safety net for the rare peak the limiter doesn't fully catch. Both
/// come after the downsampling filter on purpose: the filter can push peaks
/// higher (removing treble that was partly cancelling the bass), so anything
/// limited before it could overshoot again.
pub struct Synth {
    voices: Polyphony,
    downsampler: Downsampler2x,
    chorus: Chorus,
    limiter: Limiter,
    patch: Patch,
    /// The doubled rate the voices run at.
    internal_rate: f32,
    /// The song tempo in BPM, for the LFO's tempo sync.
    tempo: f32,
    /// The patch's own volume (from `volume_db`), as a multiplier.
    volume: f32,
    gain: f32,
    target_gain: f32,
    gain_coef: f32,
}

impl Synth {
    /// How many samples late the output is: the oversampling filter plus
    /// the limiter's lookahead.
    pub fn latency(&self) -> u32 {
        Downsampler2x::LATENCY + self.limiter.latency()
    }

    pub fn new(sample_rate: f32, patch: &Patch) -> Self {
        // Build the sine table now, so it isn't built on the audio thread the
        // first time a note plays.
        LazyLock::force(&SINE_TABLE);

        let mut synth = Self {
            voices: Polyphony::default(),
            downsampler: Downsampler2x::default(),
            chorus: Chorus::new(sample_rate),
            limiter: Limiter::new(sample_rate),
            patch: *patch,
            internal_rate: 0.0,
            tempo: 120.0,
            volume: 1.0,
            gain: 1.0,
            target_gain: 1.0,
            gain_coef: 0.0,
        };
        synth.set_sample_rate(sample_rate);
        synth.set_patch(patch);
        synth
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.internal_rate = sample_rate * 2.0;
        self.chorus = Chorus::new(sample_rate);
        self.limiter = Limiter::new(sample_rate);
        self.gain_coef = glide_coef(GAIN_GLIDE_TIME, sample_rate);
        self.voices.prepare(&self.patch, self.internal_rate);
    }

    /// Call whenever the patch may have changed (the plugin does it every
    /// few milliseconds).
    pub fn set_patch(&mut self, patch: &Patch) {
        // Switching between mono and poly: let the old notes ring out, so
        // nothing gets stuck in the wrong mode.
        if patch.mono != self.patch.mono {
            self.voices.release_all();
        }
        self.patch = *patch;
        // The voices only look at `lfo_rate`, so turn a synced division into
        // Hz here, once, for the current tempo.
        self.patch.lfo_rate = patch.lfo_hz(self.tempo);
        // Decibels to a multiplier: +6 dB ≈ ×2, -6 dB ≈ ×0.5.
        self.volume = 10f32.powf(patch.volume_db / 20.0);
        self.voices.prepare(&self.patch, self.internal_rate);
    }

    /// The song tempo in BPM. Takes effect at the next `set_patch`.
    pub fn set_tempo(&mut self, tempo: f32) {
        self.tempo = tempo;
    }

    /// Master output level (1.0 = unchanged). Changes glide smoothly.
    pub fn set_gain(&mut self, gain: f32) {
        self.target_gain = gain;
    }

    pub fn note_on(&mut self, note: u8, velocity: f32) {
        self.voices
            .note_on(note, velocity, &self.patch, self.internal_rate);
    }

    pub fn note_off(&mut self, note: u8) {
        self.voices.note_off(note, self.patch.mono);
    }

    /// Silence everything and jump straight to the target gain.
    pub fn reset(&mut self) {
        self.voices.reset();
        self.downsampler.reset();
        self.chorus.reset();
        self.limiter.reset();
        self.gain = self.target_gain;
    }

    /// Fill `left` and `right` (same length) with sound.
    pub fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
        let chunks = left.chunks_mut(MAX_BLOCK).zip(right.chunks_mut(MAX_BLOCK));
        for (left, right) in chunks {
            // Voices run at twice the rate: render 2 samples per output
            // sample, then filter them down to one.
            let mut oversampled = [0.0; MAX_BLOCK * 2];
            let oversampled = &mut oversampled[..left.len() * 2];
            self.voices.render(&self.patch, oversampled);

            for i in 0..left.len() {
                let filtered = self
                    .downsampler
                    .process(oversampled[2 * i], oversampled[2 * i + 1]);
                self.gain += (self.target_gain - self.gain) * self.gain_coef;
                let mono = filtered * self.volume * self.gain;
                let (l, r) = self.chorus.process(mono, self.patch.chorus);
                let (l, r) = self.limiter.process(l, r);
                left[i] = soft_clip(l);
                right[i] = soft_clip(r);
            }
        }
    }

    /// Compute a single (left, right) output sample. Handy for simple loops;
    /// `render` is faster for real work.
    pub fn next_sample(&mut self) -> (f32, f32) {
        let (mut left, mut right) = ([0.0], [0.0]);
        self.render(&mut left, &mut right);
        (left[0], right[0])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presets::PRESETS;

    #[test]
    fn every_preset_makes_sound_and_then_stops() {
        for (name, patch) in PRESETS {
            let mut synth = Synth::new(48_000.0, patch);
            synth.note_on(60, 1.0);
            let mut peak: f32 = 0.0;
            for _ in 0..4_800 {
                let (l, r) = synth.next_sample();
                assert!(l.is_finite() && r.is_finite(), "{name}: produced {l}, {r}");
                peak = peak.max(l.abs()).max(r.abs());
            }
            assert!(peak > 0.01, "{name}: silent");

            // The longest release in the presets is ~10 s to -60 dB.
            synth.note_off(60);
            for _ in 0..48_000 * 20 {
                synth.next_sample();
            }
            let (l, r) = synth.next_sample();
            assert!(l.abs().max(r.abs()) < 1e-4, "{name}: still sounding");
        }
    }

    #[test]
    fn extreme_settings_stay_finite() {
        let mut patch = PRESETS[0].1;
        patch.feedback = 1.0;
        for op in &mut patch.ops {
            op.ratio = 16.0;
            op.level = 1.0;
            op.attack = 0.001;
        }
        for algorithm in 1..=8 {
            patch.algorithm = algorithm;
            let mut synth = Synth::new(48_000.0, &patch);
            for note in [0, 60, 127] {
                synth.note_on(note, 1.0);
            }
            for _ in 0..4_800 {
                let (l, r) = synth.next_sample();
                assert!(l.is_finite() && r.is_finite(), "algorithm {algorithm}");
            }
        }
    }

    #[test]
    fn synced_lfo_follows_the_tempo() {
        let mut patch = PRESETS[0].1;
        patch.lfo_sync = true;
        patch.lfo_division = 6; // 1/8
        let mut synth = Synth::new(48_000.0, &patch);
        synth.set_tempo(120.0);
        synth.set_patch(&patch);
        // 120 BPM = 2 beats per second; eighth notes = 4 cycles per second.
        assert_eq!(synth.patch.lfo_rate, 4.0);
        synth.set_tempo(90.0);
        synth.set_patch(&patch);
        assert_eq!(synth.patch.lfo_rate, 3.0);
    }

    #[test]
    fn never_clips_however_hard_it_is_pushed() {
        for (name, patch) in PRESETS {
            let mut synth = Synth::new(48_000.0, patch);
            synth.set_gain(4.0); // +12 dB, way too loud on purpose
            synth.reset();
            for note in 30..90 {
                synth.note_on(note, 1.0);
            }
            for _ in 0..48_000 {
                let (l, r) = synth.next_sample();
                assert!(l.abs() < 1.0 && r.abs() < 1.0, "{name}: {l}, {r}");
            }
        }
    }

    #[test]
    fn stealing_doesnt_click() {
        // Two identical synths hold an organ chord at the voice limit. Only
        // `a` gets one more (nearly silent) note, which steals a voice. The
        // difference between them is then just the stolen voice going away:
        // a smooth ramp if it fades, a sudden jump (a click) if it's cut.
        // Full velocity sensitivity, so a velocity of 0.001 really is silent.
        let mut patch = crate::presets::find("Garage Organ").unwrap();
        patch.chorus = 0.0;
        for op in &mut patch.ops {
            op.velocity = 1.0;
        }
        let new_synth = || {
            let mut synth = Synth::new(48_000.0, &patch);
            synth.set_gain(0.1);
            synth.reset();
            for note in 0..32 {
                synth.note_on(40 + note, 1.0);
            }
            for _ in 0..4_800 {
                synth.next_sample();
            }
            synth
        };
        let (mut a, mut b) = (new_synth(), new_synth());
        a.note_on(100, 0.001);

        let mut previous = 0.0;
        let (mut largest, mut largest_step): (f32, f32) = (0.0, 0.0);
        for _ in 0..4_800 {
            let difference = a.next_sample().0 - b.next_sample().0;
            largest = largest.max(difference.abs());
            largest_step = largest_step.max((difference - previous).abs());
            previous = difference;
        }
        assert!(largest > 1e-3, "the steal should be measurable");
        assert!(
            largest_step < largest * 0.1,
            "jumped by {largest_step} at once (whole voice: {largest})"
        );
    }
}
