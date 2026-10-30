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

/// The joystick sends its position in small steps. Gliding to each new
/// position over ~10 ms turns those steps into a smooth movement; without it
/// a slow bend would sound like a staircase.
const JOYSTICK_GLIDE_TIME: f32 = 0.01;

/// The mod wheel's vibrato: a classic, singer-like speed and, with the wheel
/// all the way up, a swing of half a semitone either way.
const MOD_VIBRATO_RATE: f32 = 5.5;
const MOD_VIBRATO_DEPTH: f32 = 0.5;

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
    /// Output sample rate (the voices run at `internal_rate`, twice this).
    sample_rate: f32,
    /// Pitch bend, -1 (all the way down) to 1 (all the way up). Multiplied
    /// by the patch's `bend_range` to get semitones.
    bend: f32,
    target_bend: f32,
    /// Mod wheel, 0..1: how much vibrato to add.
    mod_wheel: f32,
    target_mod_wheel: f32,
    joystick_coef: f32,
    /// The mod wheel's own vibrato oscillator, separate from the patch's LFO
    /// so it always sounds like vibrato, whatever the preset does with its
    /// LFO. Its position in the cycle, 0..1.
    vibrato_phase: f32,
    /// Sustain pedal held down.
    pedal: bool,
    /// Keys let go while the pedal was down: they keep sounding until the
    /// pedal comes up. One flag per MIDI note number.
    sustained: [bool; 128],
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
            sample_rate,
            bend: 0.0,
            target_bend: 0.0,
            mod_wheel: 0.0,
            target_mod_wheel: 0.0,
            joystick_coef: 0.0,
            vibrato_phase: 0.0,
            pedal: false,
            sustained: [false; 128],
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
        self.joystick_coef = glide_coef(JOYSTICK_GLIDE_TIME, sample_rate);
        self.sample_rate = sample_rate;
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
        // Played again: from now on it's held by the key, not the pedal.
        self.sustained[note as usize] = false;
        self.voices
            .note_on(note, velocity, &self.patch, self.internal_rate);
    }

    pub fn note_off(&mut self, note: u8) {
        if self.pedal {
            // The pedal keeps it sounding; it's released with the pedal.
            self.sustained[note as usize] = true;
            return;
        }
        self.voices.note_off(note, self.patch.mono);
    }

    /// Sustain pedal down or up. While it's down, letting go of a key doesn't
    /// end the note; lifting the pedal releases every note held that way.
    pub fn set_sustain(&mut self, down: bool) {
        self.pedal = down;
        if down {
            return;
        }
        // In mono mode, releasing the sounding note slides back to another
        // key that's still down. So let go of the silent ones first: else it
        // could briefly slide to one of them on its way out.
        let sounding = self.patch.mono.then(|| self.voices.mono_note() as usize);
        for note in (0..128).filter(|&n| Some(n) != sounding).chain(sounding) {
            if self.sustained[note] {
                self.sustained[note] = false;
                self.voices.note_off(note as u8, self.patch.mono);
            }
        }
    }

    /// "All notes off" (sent by DAWs on stop, and by panic buttons): every
    /// note fades out with its normal release.
    pub fn all_notes_off(&mut self) {
        self.pedal = false;
        self.sustained = [false; 128];
        self.voices.release_all();
    }

    /// "All sound off": silence right now, no release tails.
    pub fn all_sound_off(&mut self) {
        self.pedal = false;
        self.sustained = [false; 128];
        self.voices.reset();
    }

    /// "Reset all controllers": joystick centered, pedal up.
    pub fn reset_controllers(&mut self) {
        self.set_pitch_bend(0.0);
        self.set_mod_wheel(0.0);
        self.set_sustain(false);
    }

    /// Pitch bend (the joystick's left/right), -1..1, 0 = centered. Bends
    /// every note, including ones that are already playing.
    pub fn set_pitch_bend(&mut self, bend: f32) {
        self.target_bend = bend.clamp(-1.0, 1.0);
    }

    /// Mod wheel (the joystick's up/down), 0..1: adds vibrato.
    pub fn set_mod_wheel(&mut self, amount: f32) {
        self.target_mod_wheel = amount.clamp(0.0, 1.0);
    }

    /// How much to scale every frequency right now, from the pitch bend and
    /// the mod wheel's vibrato. `samples` is how far time moves on.
    fn joystick_pitch(&mut self, samples: usize) -> f32 {
        for _ in 0..samples {
            self.bend += (self.target_bend - self.bend) * self.joystick_coef;
            self.mod_wheel += (self.target_mod_wheel - self.mod_wheel) * self.joystick_coef;
        }
        self.vibrato_phase += MOD_VIBRATO_RATE * samples as f32 / self.sample_rate;
        self.vibrato_phase -= self.vibrato_phase.floor();
        let vibrato = (self.vibrato_phase * std::f32::consts::TAU).sin();

        let semitones =
            self.bend * self.patch.bend_range + vibrato * self.mod_wheel * MOD_VIBRATO_DEPTH;
        // Same rule as for notes: +12 semitones doubles the frequency.
        (semitones / 12.0).exp2()
    }

    /// Silence everything and jump straight to the target gain.
    pub fn reset(&mut self) {
        self.all_sound_off();
        self.downsampler.reset();
        self.chorus.reset();
        self.limiter.reset();
        self.gain = self.target_gain;
        self.bend = self.target_bend;
        self.mod_wheel = self.target_mod_wheel;
    }

    /// Fill `left` and `right` (same length) with sound.
    pub fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
        let chunks = left.chunks_mut(MAX_BLOCK).zip(right.chunks_mut(MAX_BLOCK));
        for (left, right) in chunks {
            // Voices run at twice the rate: render 2 samples per output
            // sample, then filter them down to one.
            let mut oversampled = [0.0; MAX_BLOCK * 2];
            let oversampled = &mut oversampled[..left.len() * 2];
            // The joystick changes slowly, so once per chunk is often enough.
            let bend = self.joystick_pitch(left.len());
            self.voices.render(&self.patch, bend, oversampled);

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

    /// Count upward zero crossings of the left channel over `samples`.
    fn zero_crossings(synth: &mut Synth, samples: usize) -> usize {
        let mut previous = 0.0;
        let mut count = 0;
        for _ in 0..samples {
            let (l, _) = synth.next_sample();
            if previous <= 0.0 && l > 0.0 {
                count += 1;
            }
            previous = l;
        }
        count
    }

    #[test]
    fn pitch_bend_moves_the_pitch_by_the_bend_range() {
        // A plain sine (op 1 only), so zero crossings give the frequency.
        let mut patch = crate::presets::find("Garage Organ").unwrap();
        patch.algorithm = 8;
        patch.chorus = 0.0;
        patch.feedback = 0.0;
        patch.lfo_pitch = 0.0;
        patch.bend_range = 12.0;
        patch.ops[0].ratio = 1.0;
        for op in &mut patch.ops[1..] {
            op.level = 0.0;
        }
        let mut synth = Synth::new(48_000.0, &patch);
        synth.note_on(57, 1.0); // 220 Hz
        synth.set_pitch_bend(1.0); // a full octave up: 440 Hz
        zero_crossings(&mut synth, 4_800); // let the bend glide there
        let crossings = zero_crossings(&mut synth, 48_000);
        assert!((438..=442).contains(&crossings), "{crossings} Hz");
    }

    #[test]
    fn sustain_pedal_holds_notes_until_lifted() {
        let patch = PRESETS[0].1;
        let mut synth = Synth::new(48_000.0, &patch);
        synth.set_sustain(true);
        synth.note_on(60, 1.0);
        synth.note_off(60);
        let loudest = |synth: &mut Synth, samples: usize| {
            let mut peak: f32 = 0.0;
            for _ in 0..samples {
                peak = peak.max(synth.next_sample().0.abs());
            }
            peak
        };
        // Still sounding 5 s after the key was let go...
        loudest(&mut synth, 48_000 * 4);
        assert!(loudest(&mut synth, 48_000) > 0.01);
        // ...and it fades out once the pedal comes up.
        synth.set_sustain(false);
        loudest(&mut synth, 48_000 * 20);
        assert!(loudest(&mut synth, 4_800) < 1e-4);
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
