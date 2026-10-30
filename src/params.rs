//! The parameters the DAW sees (and can automate or map to knobs).
//!
//! They mirror the `Patch` struct one-to-one: `new` builds them from a patch,
//! `to_patch` reads their current values back into one, and `load_patch`
//! (used by the GUI's preset menu) sets them all to a patch's values.
//!
//! Parameters that change the sound continuously (levels, ratios, feedback)
//! are *smoothed*: when a knob jumps, the value glides to it over a few
//! milliseconds instead. Without that you'd hear "zipper noise", a crackle
//! from the sound changing in steps.

use crate::dsp::algorithm::ALGORITHMS;
use crate::dsp::vowel;
use crate::dsp::{FilterPatch, LFO_DIVISIONS, NUM_OPS, OpPatch, Patch};
use nih_plug::prelude::*;
use nih_plug_egui::EguiState;
use std::sync::{Arc, Mutex};

/// Where the Gain knob starts. Measured: at -15 dB normal playing never
/// reaches the limiter, and only very hard, full chords on the brightest
/// presets get turned down a little. Louder than that, the limiter starts
/// working (audibly) on ordinary chords.
pub const DEFAULT_GAIN_DB: f32 = -15.0;

#[derive(Params)]
pub struct SynthParams {
    /// GUI window size. `persist` fields are saved with the DAW project but
    /// aren't parameters: the DAW can't automate them.
    #[persist = "editor-state"]
    pub editor_state: Arc<EguiState>,
    /// Name of the last loaded preset, shown in the GUI's preset menu. Only
    /// the GUI uses it, never the audio thread, so a Mutex is fine here.
    #[persist = "preset"]
    pub preset_name: Mutex<String>,

    #[id = "gain"]
    pub gain: FloatParam,
    #[id = "algorithm"]
    pub algorithm: IntParam,
    #[id = "feedback"]
    pub feedback: FloatParam,
    #[id = "key_track"]
    pub key_track: FloatParam,
    #[id = "volume"]
    pub volume: FloatParam,
    #[id = "mono"]
    pub mono: BoolParam,
    #[id = "glide"]
    pub glide: FloatParam,
    #[id = "lfo_rate"]
    pub lfo_rate: FloatParam,
    #[id = "lfo_sync"]
    pub lfo_sync: BoolParam,
    #[id = "lfo_division"]
    pub lfo_division: IntParam,
    #[id = "lfo_pitch"]
    pub lfo_pitch: FloatParam,
    #[id = "lfo_wobble"]
    pub lfo_wobble: FloatParam,
    #[id = "lfo_volume"]
    pub lfo_volume: FloatParam,
    #[id = "lfo_cutoff"]
    pub lfo_cutoff: FloatParam,
    #[id = "chorus"]
    pub chorus: FloatParam,
    #[id = "vowel"]
    pub vowel: FloatParam,
    #[id = "vowel_mix"]
    pub vowel_mix: FloatParam,
    /// Shown in the DAW as the group "Filter".
    #[nested(group = "Filter")]
    pub filter: FilterParams,
    /// Shown in the DAW as groups "Operator 1" to "Operator 4".
    #[nested(array, group = "Operator")]
    pub ops: [OpParams; NUM_OPS],
}

#[derive(Params)]
pub struct FilterParams {
    #[id = "filter_cutoff"]
    pub cutoff: FloatParam,
    #[id = "filter_resonance"]
    pub resonance: FloatParam,
    #[id = "filter_env"]
    pub env_amount: FloatParam,
    #[id = "filter_attack"]
    pub attack: FloatParam,
    #[id = "filter_decay"]
    pub decay: FloatParam,
    #[id = "filter_sustain"]
    pub sustain: FloatParam,
    #[id = "filter_release"]
    pub release: FloatParam,
    #[id = "filter_key_track"]
    pub key_track: FloatParam,
    #[id = "filter_velocity"]
    pub velocity: FloatParam,
}

#[derive(Params)]
pub struct OpParams {
    #[id = "ratio"]
    pub ratio: FloatParam,
    #[id = "level"]
    pub level: FloatParam,
    #[id = "velocity"]
    pub velocity: FloatParam,
    #[id = "attack"]
    pub attack: FloatParam,
    #[id = "decay"]
    pub decay: FloatParam,
    #[id = "sustain"]
    pub sustain: FloatParam,
    #[id = "release"]
    pub release: FloatParam,
}

impl SynthParams {
    pub fn new(preset_name: &str, patch: &Patch) -> Self {
        Self {
            editor_state: EguiState::from_size(800, 790),
            preset_name: Mutex::new(preset_name.to_string()),
            gain: FloatParam::new(
                "Gain",
                util::db_to_gain(DEFAULT_GAIN_DB),
                FloatRange::Skewed {
                    min: util::db_to_gain(-36.0),
                    max: util::db_to_gain(0.0),
                    factor: FloatRange::gain_skew_factor(-36.0, 0.0),
                },
            )
            // No smoother here: `Synth::set_gain` glides by itself.
            .with_unit(" dB")
            .with_value_to_string(formatters::v2s_f32_gain_to_db(1))
            .with_string_to_value(formatters::s2v_f32_gain_to_db()),
            algorithm: IntParam::new(
                "Algorithm",
                patch.algorithm as i32,
                IntRange::Linear {
                    min: 1,
                    max: ALGORITHMS.len() as i32,
                },
            )
            .with_value_to_string(Arc::new(|n| {
                format!("{n}: {}", ALGORITHMS[n as usize - 1].name)
            })),
            feedback: amount_param("Feedback", patch.feedback).with_smoother(SMOOTHING),
            key_track: amount_param("Key Track", patch.key_track),
            volume: FloatParam::new(
                "Preset Volume",
                patch.volume_db,
                FloatRange::Linear {
                    min: -24.0,
                    max: 24.0,
                },
            )
            .with_smoother(SMOOTHING)
            .with_unit(" dB")
            .with_value_to_string(formatters::v2s_f32_rounded(1)),
            mono: BoolParam::new("Mono", patch.mono),
            glide: time_param("Glide", patch.glide, 0.0, 2.0),
            lfo_rate: FloatParam::new(
                "LFO Rate",
                patch.lfo_rate,
                FloatRange::Skewed {
                    min: 0.1,
                    max: 20.0,
                    factor: FloatRange::skew_factor(-1.0),
                },
            )
            .with_unit(" Hz")
            .with_value_to_string(formatters::v2s_f32_rounded(2)),
            lfo_sync: BoolParam::new("LFO Sync", patch.lfo_sync),
            lfo_division: IntParam::new(
                "LFO Division",
                patch.lfo_division as i32,
                IntRange::Linear {
                    min: 0,
                    max: LFO_DIVISIONS.len() as i32 - 1,
                },
            )
            .with_value_to_string(Arc::new(|i| LFO_DIVISIONS[i as usize].0.to_string())),
            lfo_pitch: FloatParam::new(
                "LFO Pitch",
                patch.lfo_pitch,
                FloatRange::Linear { min: 0.0, max: 2.0 },
            )
            .with_smoother(SMOOTHING)
            .with_unit(" st")
            .with_value_to_string(formatters::v2s_f32_rounded(2)),
            lfo_wobble: amount_param("LFO Wobble", patch.lfo_wobble).with_smoother(SMOOTHING),
            lfo_volume: amount_param("LFO Volume", patch.lfo_volume).with_smoother(SMOOTHING),
            lfo_cutoff: amount_param("LFO Cutoff", patch.lfo_cutoff).with_smoother(SMOOTHING),
            chorus: amount_param("Chorus", patch.chorus).with_smoother(SMOOTHING),
            vowel: FloatParam::new(
                "Vowel",
                patch.vowel,
                FloatRange::Linear { min: 0.0, max: 4.0 },
            )
            .with_smoother(SMOOTHING)
            .with_value_to_string(Arc::new(vowel::describe)),
            vowel_mix: amount_param("Vowel Mix", patch.vowel_mix).with_smoother(SMOOTHING),
            filter: FilterParams::new(&patch.filter),
            ops: std::array::from_fn(|i| OpParams::new(i + 1, &patch.ops[i])),
        }
    }

    /// Read the current values. `steps` is how many samples have passed since
    /// the last call, to move the smoothed parameters along by that much.
    pub fn to_patch(&self, steps: u32) -> Patch {
        Patch {
            algorithm: self.algorithm.value() as usize,
            feedback: self.feedback.smoothed.next_step(steps),
            key_track: self.key_track.value(),
            volume_db: self.volume.smoothed.next_step(steps),
            mono: self.mono.value(),
            glide: self.glide.value(),
            lfo_rate: self.lfo_rate.value(),
            lfo_sync: self.lfo_sync.value(),
            lfo_division: self.lfo_division.value() as usize,
            lfo_pitch: self.lfo_pitch.smoothed.next_step(steps),
            lfo_wobble: self.lfo_wobble.smoothed.next_step(steps),
            lfo_volume: self.lfo_volume.smoothed.next_step(steps),
            lfo_cutoff: self.lfo_cutoff.smoothed.next_step(steps),
            chorus: self.chorus.smoothed.next_step(steps),
            vowel: self.vowel.smoothed.next_step(steps),
            vowel_mix: self.vowel_mix.smoothed.next_step(steps),
            filter: self.filter.to_patch(steps),
            ops: std::array::from_fn(|i| self.ops[i].to_patch(steps)),
        }
    }

    /// Set every parameter to the patch's values (gain is left alone). Going
    /// through the `ParamSetter` tells the DAW about each change, so it shows
    /// up in undo history and automation.
    pub fn load_patch(&self, setter: &ParamSetter, patch: &Patch) {
        set(setter, &self.algorithm, patch.algorithm as i32);
        set(setter, &self.feedback, patch.feedback);
        set(setter, &self.key_track, patch.key_track);
        set(setter, &self.volume, patch.volume_db);
        set(setter, &self.mono, patch.mono);
        set(setter, &self.glide, patch.glide);
        set(setter, &self.lfo_rate, patch.lfo_rate);
        set(setter, &self.lfo_sync, patch.lfo_sync);
        set(setter, &self.lfo_division, patch.lfo_division as i32);
        set(setter, &self.lfo_pitch, patch.lfo_pitch);
        set(setter, &self.lfo_wobble, patch.lfo_wobble);
        set(setter, &self.lfo_volume, patch.lfo_volume);
        set(setter, &self.lfo_cutoff, patch.lfo_cutoff);
        set(setter, &self.chorus, patch.chorus);
        set(setter, &self.vowel, patch.vowel);
        set(setter, &self.vowel_mix, patch.vowel_mix);
        let (f, fp) = (&self.filter, &patch.filter);
        set(setter, &f.cutoff, fp.cutoff);
        set(setter, &f.resonance, fp.resonance);
        set(setter, &f.env_amount, fp.env_amount);
        set(setter, &f.attack, fp.attack);
        set(setter, &f.decay, fp.decay);
        set(setter, &f.sustain, fp.sustain);
        set(setter, &f.release, fp.release);
        set(setter, &f.key_track, fp.key_track);
        set(setter, &f.velocity, fp.velocity);
        for (op_params, op) in self.ops.iter().zip(&patch.ops) {
            set(setter, &op_params.ratio, op.ratio);
            set(setter, &op_params.level, op.level);
            set(setter, &op_params.velocity, op.velocity);
            set(setter, &op_params.attack, op.attack);
            set(setter, &op_params.decay, op.decay);
            set(setter, &op_params.sustain, op.sustain);
            set(setter, &op_params.release, op.release);
        }
    }
}

/// Change one parameter the way a DAW expects: begin, set, end.
pub(crate) fn set<P: Param>(setter: &ParamSetter, param: &P, value: P::Plain) {
    setter.begin_set_parameter(param);
    setter.set_parameter(param, value);
    setter.end_set_parameter(param);
}

impl FilterParams {
    fn new(f: &FilterPatch) -> Self {
        Self {
            cutoff: FloatParam::new(
                "Filter Cutoff",
                f.cutoff,
                FloatRange::Skewed {
                    min: 20.0,
                    max: 20_000.0,
                    factor: FloatRange::skew_factor(-2.0),
                },
            )
            // Logarithmic: a sweep sounds even, octave by octave.
            .with_smoother(SmoothingStyle::Logarithmic(20.0))
            .with_value_to_string(formatters::v2s_f32_hz_then_khz(0))
            .with_string_to_value(formatters::s2v_f32_hz_then_khz()),
            resonance: amount_param("Filter Resonance", f.resonance).with_smoother(SMOOTHING),
            env_amount: amount_param("Filter Env Amount", f.env_amount).with_smoother(SMOOTHING),
            attack: time_param("Filter Attack", f.attack, 0.001, 5.0),
            decay: time_param("Filter Decay", f.decay, 0.01, 20.0),
            sustain: amount_param("Filter Sustain", f.sustain),
            release: time_param("Filter Release", f.release, 0.01, 10.0),
            key_track: amount_param("Filter Key Track", f.key_track),
            velocity: amount_param("Filter Velocity", f.velocity),
        }
    }

    fn to_patch(&self, steps: u32) -> FilterPatch {
        FilterPatch {
            cutoff: self.cutoff.smoothed.next_step(steps),
            resonance: self.resonance.smoothed.next_step(steps),
            env_amount: self.env_amount.smoothed.next_step(steps),
            attack: self.attack.value(),
            decay: self.decay.value(),
            sustain: self.sustain.value(),
            release: self.release.value(),
            key_track: self.key_track.value(),
            velocity: self.velocity.value(),
        }
    }
}

impl OpParams {
    fn new(number: usize, op: &OpPatch) -> Self {
        let name = |what: &str| format!("Op {number} {what}");
        Self {
            ratio: FloatParam::new(
                name("Ratio"),
                op.ratio,
                FloatRange::Skewed {
                    min: 0.5,
                    max: 16.0,
                    factor: FloatRange::skew_factor(-1.0),
                },
            )
            // Logarithmic: a glide from ratio 1 to 2 sounds like an even pitch sweep.
            .with_smoother(SmoothingStyle::Logarithmic(20.0))
            .with_value_to_string(formatters::v2s_f32_rounded(3)),
            level: amount_param(&name("Level"), op.level).with_smoother(SMOOTHING),
            velocity: amount_param(&name("Velocity"), op.velocity),
            attack: time_param(&name("Attack"), op.attack, 0.001, 5.0),
            decay: time_param(&name("Decay"), op.decay, 0.01, 60.0),
            sustain: amount_param(&name("Sustain"), op.sustain),
            release: time_param(&name("Release"), op.release, 0.01, 30.0),
        }
    }

    fn to_patch(&self, steps: u32) -> OpPatch {
        OpPatch {
            ratio: self.ratio.smoothed.next_step(steps),
            level: self.level.smoothed.next_step(steps),
            velocity: self.velocity.value(),
            attack: self.attack.value(),
            decay: self.decay.value(),
            sustain: self.sustain.value(),
            release: self.release.value(),
        }
    }
}

/// How fast smoothed parameters follow a change: over 20 ms.
const SMOOTHING: SmoothingStyle = SmoothingStyle::Linear(20.0);

/// A plain 0..1 value.
fn amount_param(name: &str, default: f32) -> FloatParam {
    FloatParam::new(name, default, FloatRange::Linear { min: 0.0, max: 1.0 })
        .with_value_to_string(formatters::v2s_f32_rounded(2))
}

/// A time in seconds, skewed so the short end gets most of the knob's travel.
fn time_param(name: &str, default: f32, min: f32, max: f32) -> FloatParam {
    FloatParam::new(
        name,
        default,
        FloatRange::Skewed {
            min,
            max,
            factor: FloatRange::skew_factor(-2.0),
        },
    )
    .with_unit(" s")
    .with_value_to_string(formatters::v2s_f32_rounded(3))
}
