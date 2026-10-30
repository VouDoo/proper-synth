use super::algorithm::ALGORITHMS;
use super::envelope::{Adsr, glide_coef};
use super::filter::{Coefs, Svf};
use super::operator::{Operator, phase_increment};
use super::patch::{NUM_OPS, Patch};
use super::vowel::VowelFilter;

/// The Filter Cutoff knob's top value: there, the filter is switched off.
const FILTER_MAX: f32 = 20_000.0;

/// How long a stolen voice takes to fade out. Short enough that you don't
/// hear it as a fade, long enough not to click.
const STEAL_FADE_TIME: f32 = 0.003;

/// One playing note: 4 operators, each with its own envelope, then a filter
/// and the vowel, plus an LFO.
///
/// ```text
///   operators ─▶ filter ─▶ vowel ─▶ × tremolo ─▶ out
/// ```
///
/// Work is split in two:
/// - `prepare` turns the patch into per-sample numbers (pitch increments,
///   envelope steps, levels). It uses slow math like `powf`, but only runs
///   once per block of samples.
/// - `next_sample` runs every sample and only does cheap math.
#[derive(Debug, Clone, Default)]
pub struct Voice {
    pub note: u8,
    /// When this voice was started; used to pick the oldest voice to steal.
    pub age: u64,
    velocity: f32,
    /// Current pitch as a (fractional) MIDI note number. Normally equal to
    /// `target_pitch`; during a mono glide it slides toward it.
    pitch: f32,
    target_pitch: f32,
    glide_coef: f32,
    sample_rate: f32,
    ops: [Operator; NUM_OPS],
    envs: [Adsr; NUM_OPS],
    ratios: [f32; NUM_OPS],
    /// Per operator: how much phase to advance each sample.
    increments: [u32; NUM_OPS],
    /// Per operator: output level, including velocity.
    levels: [f32; NUM_OPS],
    /// Operator 4's last two outputs, used for feedback.
    feedback: [f32; 2],
    /// The LFO is just another sine oscillator, running very slowly.
    lfo: Operator,
    lfo_increment: u32,
    filter: Svf,
    filter_env: Adsr,
    /// This note's cutoff before the envelope and LFO: the patch's cutoff
    /// with key tracking and velocity applied.
    filter_cutoff: f32,
    filter_q: f32,
    max_cutoff: f32,
    /// False when the Cutoff knob is all the way up and nothing moves it:
    /// then the filter is skipped, leaving the sound untouched and saving CPU.
    filter_on: bool,
    vowel: VowelFilter,
    /// Set when this voice was stolen: it's fading out to make room.
    fading: bool,
    fade_gain: f32,
    fade_step: f32,
}

impl Voice {
    pub fn note_on(&mut self, note: u8, velocity: f32, age: u64) {
        if !self.is_active() {
            // Fresh voice: start every operator from phase 0, so each note
            // sounds the same. (When re-triggering a sounding voice we keep
            // the phases, so the waveform doesn't jump and click.)
            for op in &mut self.ops {
                op.reset();
            }
            self.feedback = [0.0; 2];
            self.filter.reset();
            self.vowel.reset();
            // Restart the LFO too, so a wobble always begins the same way and
            // stays in time with your playing. (Re-striking a ringing note
            // keeps it running: jumping would click, e.g. with tremolo.)
            self.lfo.reset();
        }

        self.note = note;
        self.age = age;
        self.velocity = velocity;
        self.pitch = note as f32;
        self.target_pitch = note as f32;
        self.fading = false;
        self.fade_gain = 1.0;

        for env in &mut self.envs {
            env.note_on();
        }
        self.filter_env.note_on();
    }

    /// Slide to another note without restarting it (mono legato).
    pub fn glide_to(&mut self, note: u8) {
        self.note = note;
        self.target_pitch = note as f32;
    }

    pub fn note_off(&mut self) {
        for env in &mut self.envs {
            env.note_off();
        }
        self.filter_env.note_off();
    }

    /// Fade out quickly and go silent, instead of stopping dead (which
    /// would click). Used when this voice is stolen for a new note.
    pub fn fade_out(&mut self, sr: f32) {
        if !self.fading {
            self.fading = true;
            self.fade_step = 1.0 / (STEAL_FADE_TIME * sr);
        }
    }

    /// Making any sound, including fading out.
    pub fn is_active(&self) -> bool {
        self.envs.iter().any(|env| env.is_active())
    }

    /// Playing a note that's still wanted (active and not fading out).
    pub fn is_playing(&self) -> bool {
        self.is_active() && !self.fading
    }

    pub fn reset(&mut self) {
        for env in &mut self.envs {
            env.reset();
        }
        self.filter_env.reset();
        self.fading = false;
    }

    /// Turn the patch into the per-sample numbers `next_sample` needs.
    pub fn prepare(&mut self, patch: &Patch, sr: f32) {
        self.sample_rate = sr;
        // Same exponential curve as the envelopes: within 60 dB of the
        // target note after `glide` seconds. 0 = jump straight there.
        self.glide_coef = glide_coef(patch.glide, sr);
        self.lfo_increment = phase_increment(patch.lfo_rate, sr);

        // Key tracking: how many octaves above middle C (note 60) we are,
        // turned into a time multiplier. 1 octave up with full key tracking
        // = half the decay time.
        let octaves = (self.note as f32 - 60.0) / 12.0;
        let time_scale = 2f32.powf(-patch.key_track * octaves);

        for i in 0..NUM_OPS {
            let op = &patch.ops[i];
            self.ratios[i] = op.ratio;
            self.envs[i].set(
                op.attack,
                op.decay * time_scale,
                op.sustain,
                op.release * time_scale,
                sr,
            );
            // velocity = 1 (hardest hit) always gives full level; softer hits
            // lower the level as much as the sensitivity allows.
            self.levels[i] = op.level * (1.0 - op.velocity * (1.0 - self.velocity));
        }
        self.update_increments();

        let f = &patch.filter;
        self.filter_env.set(
            f.attack,
            f.decay * time_scale,
            f.sustain,
            f.release * time_scale,
            sr,
        );
        // Resonance 0..1 as a "q": 0.7 is smooth, 15.7 is a sharp peak.
        self.filter_q = 0.7 + 15.0 * f.resonance * f.resonance;
        self.max_cutoff = (0.45 * sr).min(20_000.0);
        // Key tracking and velocity, in octaves: higher notes open the filter
        // (as much as `key_track` says), softer hits close it.
        let octaves = f.key_track * (self.note as f32 - 60.0) / 12.0
            - f.velocity * (1.0 - self.velocity) * 3.0;
        self.filter_cutoff = f.cutoff * octaves.exp2();
        // Skip the filter only when its knob is all the way up and the LFO
        // doesn't move it. (Deciding from the knob, not from this note's
        // cutoff, means key tracking or a glide can't switch it on and off
        // mid-note, which would click.)
        self.filter_on = f.cutoff < FILTER_MAX || patch.lfo_cutoff > 0.0;
        if !self.filter_on {
            // Start from silence when it's switched back on.
            self.filter.reset();
        }

        if patch.vowel_mix > 0.0 {
            self.vowel.set(patch.vowel, sr);
        }
    }

    /// Recompute each operator's phase increment from the current pitch.
    fn update_increments(&mut self) {
        // MIDI note 69 is A4 = 440 Hz; every 12 notes up doubles the frequency.
        let freq = 440.0 * 2f32.powf((self.pitch - 69.0) / 12.0);
        for i in 0..NUM_OPS {
            self.increments[i] = phase_increment(freq * self.ratios[i], self.sample_rate);
        }
    }

    /// Compute a block of samples, *adding* them into `out` (other voices
    /// are already in there).
    pub fn render(&mut self, patch: &Patch, out: &mut [f32]) {
        for sample in out {
            *sample += self.next_sample(patch);
        }
    }

    /// Compute one sample.
    #[inline(always)]
    pub fn next_sample(&mut self, patch: &Patch) -> f32 {
        let algorithm = &ALGORITHMS[patch.algorithm - 1];

        // Glide: move the pitch a step toward the target note.
        if self.pitch != self.target_pitch {
            self.pitch += (self.target_pitch - self.pitch) * self.glide_coef;
            if (self.target_pitch - self.pitch).abs() < 0.001 {
                self.pitch = self.target_pitch;
            }
            self.update_increments();
        }

        // The LFO's three jobs. `lfo` swings between -1 and 1.
        let lfo = self.lfo.next_sample(self.lfo_increment, 0.0);
        // Vibrato: for small pitch changes, raising the pitch by s semitones
        // is very nearly multiplying the frequency by 1 + 0.0578·s.
        let vibrato = 1.0 + lfo * patch.lfo_pitch * 0.057_762;
        // Wobble: scale the modulation between 0 and 2 times its normal amount.
        let wobble = (1.0 + lfo * patch.lfo_wobble).max(0.0);
        // Tremolo: full volume at the top of the swing, quieter at the bottom.
        let tremolo = 1.0 - patch.lfo_volume * 0.5 * (1.0 - lfo);

        // Each operator's output this sample, filled in as we go.
        let mut out = [0.0; NUM_OPS];

        // Go from operator 4 down to 1, so modulators are always computed
        // before the operators they modulate (see `algorithm.rs`).
        for i in (0..NUM_OPS).rev() {
            // Add up everything modulating this operator. Outputs are used
            // directly as a phase offset in cycles, so a modulator at level
            // 1.0 swings the phase by a full cycle (a modulation index of 2π).
            let mut phase_mod = 0.0;
            for &m in algorithm.modulators[i] {
                phase_mod += out[m];
            }
            // Operator 4 also listens to itself. Averaging the last two
            // samples keeps the feedback loop from getting unstable.
            if i == NUM_OPS - 1 {
                phase_mod += patch.feedback * 0.5 * (self.feedback[0] + self.feedback[1]) / 2.0;
            }

            let increment = (self.increments[i] as f32 * vibrato) as u32;
            let env = self.envs[i].next_sample();
            let sine = self.ops[i].next_sample(increment, phase_mod * wobble);
            out[i] = sine * env * self.levels[i];
        }

        self.feedback = [out[NUM_OPS - 1], self.feedback[0]];

        // Mix the carriers. Dividing by their count keeps algorithms with
        // several carriers from being much louder than those with one.
        let mut sum = 0.0;
        for &c in algorithm.carriers {
            sum += out[c];
        }
        let mut sample = sum / algorithm.carriers.len() as f32;

        // The filter. Its envelope and the LFO move the cutoff in octaves,
        // so a sweep sounds even from bottom to top.
        let filter_env = self.filter_env.next_sample();
        if self.filter_on {
            let octaves = filter_env * patch.filter.env_amount * 7.0 + lfo * patch.lfo_cutoff * 3.0;
            let cutoff = (self.filter_cutoff * octaves.exp2()).clamp(20.0, self.max_cutoff);
            let coefs = Coefs::new(cutoff, self.filter_q, self.sample_rate);
            sample = self.filter.process(sample, &coefs).0;
        }

        if patch.vowel_mix > 0.0 {
            let sung = self.vowel.process(sample);
            sample += (sung - sample) * patch.vowel_mix;
        }

        sample *= tremolo;

        if self.fading {
            self.fade_gain -= self.fade_step;
            if self.fade_gain <= 0.0 {
                self.reset();
                return 0.0;
            }
            sample *= self.fade_gain;
        }
        sample
    }
}
