/// Number of operators per voice.
pub const NUM_OPS: usize = 4;

/// Note lengths the LFO can sync to, with how many beats one LFO cycle
/// lasts. "." is dotted (1.5× longer), "T" is a triplet (3 in the time of 2).
pub const LFO_DIVISIONS: [(&str, f32); 11] = [
    ("1/1", 4.0),
    ("1/2", 2.0),
    ("1/4.", 1.5),
    ("1/4", 1.0),
    ("1/4T", 2.0 / 3.0),
    ("1/8.", 0.75),
    ("1/8", 0.5),
    ("1/8T", 1.0 / 3.0),
    ("1/16", 0.25),
    ("1/16T", 1.0 / 6.0),
    ("1/32", 0.125),
];

/// Settings for one operator.
#[derive(Debug, Clone, Copy)]
pub struct OpPatch {
    /// Frequency as a multiple of the note's frequency. 1 = the note itself,
    /// 2 = an octave up, 0.5 = an octave down, 3.5 = something bell-like.
    pub ratio: f32,
    /// Output level 0..1. For a carrier this is volume; for a modulator it's
    /// how much it changes the timbre (1.0 = a modulation index of 2π).
    pub level: f32,
    /// Velocity sensitivity 0..1. 0 = always full level, 1 = level follows how
    /// hard you hit the key. On a modulator this makes hard hits brighter.
    pub velocity: f32,
    /// Envelope, in seconds (sustain is a level 0..1).
    pub attack: f32,
    pub decay: f32,
    pub sustain: f32,
    pub release: f32,
}

/// The resonant lowpass filter each note goes through (see `filter.rs`).
#[derive(Debug, Clone, Copy)]
pub struct FilterPatch {
    /// Cutoff in Hz for middle C at full velocity, before the envelope opens
    /// it further. 20000 = fully open (the filter is then skipped entirely).
    pub cutoff: f32,
    /// Resonance 0..1: from no peak to a sharp, squelchy one at the cutoff.
    pub resonance: f32,
    /// How far the filter envelope opens the cutoff, 0..1 (1 = 7 octaves).
    pub env_amount: f32,
    /// The filter's own envelope (times in seconds, sustain 0..1).
    pub attack: f32,
    pub decay: f32,
    pub sustain: f32,
    pub release: f32,
    /// 0..1: how much the cutoff follows the note. At 1 every note has the
    /// same brightness; at 0 high notes sound darker than low ones.
    pub key_track: f32,
    /// 0..1: how much softer hits close the filter (up to 3 octaves).
    pub velocity: f32,
}

/// A filter that does nothing: fully open, no envelope.
pub const FILTER_OPEN: FilterPatch = FilterPatch {
    cutoff: 20_000.0,
    resonance: 0.0,
    env_amount: 0.0,
    attack: 0.001,
    decay: 0.5,
    sustain: 1.0,
    release: 0.3,
    key_track: 0.0,
    velocity: 0.0,
};

/// Everything that defines one sound.
#[derive(Debug, Clone, Copy)]
pub struct Patch {
    /// Algorithm number, 1-8 (see `algorithm.rs`).
    pub algorithm: usize,
    /// How much operator 4 modulates *itself* (0..1). Self-modulation turns a
    /// sine into something close to a sawtooth: buzzy, great for bass and brass.
    pub feedback: f32,
    /// Key tracking 0..1: how much shorter decay/release get for higher notes.
    /// Real instruments work like this: bass strings ring much longer.
    pub key_track: f32,
    /// Overall loudness of this sound, in dB. Used to make every preset about
    /// equally loud, so switching between them doesn't jump in volume.
    pub volume_db: f32,

    /// Mono: one note at a time, like a bass synth. Pressing a new key while
    /// still holding the previous one slides to it ("legato") instead of
    /// restarting the note.
    pub mono: bool,
    /// How long a mono slide takes, in seconds (0 = jump straight there).
    pub glide: f32,

    /// The LFO ("low frequency oscillator") is a slow sine wave that moves
    /// things up and down over time. Its speed, in Hz (cycles per second):
    pub lfo_rate: f32,
    /// Follow the song's tempo instead of `lfo_rate`, so wobbles land on the
    /// beat: one LFO cycle lasts `LFO_DIVISIONS[lfo_division]`.
    pub lfo_sync: bool,
    pub lfo_division: usize,
    /// Vibrato: how far the pitch swings, in semitones.
    pub lfo_pitch: f32,
    /// Wobble 0..1: how much the brightness (the modulation) swings.
    pub lfo_wobble: f32,
    /// Tremolo 0..1: how much the volume swings.
    pub lfo_volume: f32,
    /// Filter wobble 0..1: how far the LFO moves the cutoff (1 = ±3 octaves).
    pub lfo_cutoff: f32,

    pub filter: FilterPatch,

    /// Which vowel the sound sings, 0.0 (A) to 4.0 (U); see `vowel.rs`.
    pub vowel: f32,
    /// How much of the vowel-filtered sound to use, 0..1 (0 = off).
    pub vowel_mix: f32,

    /// Stereo chorus amount 0..1 (see `chorus.rs`).
    pub chorus: f32,

    pub ops: [OpPatch; NUM_OPS],
}

impl Patch {
    /// The LFO speed in Hz, following `tempo` (in BPM) when synced.
    pub fn lfo_hz(&self, tempo: f32) -> f32 {
        if self.lfo_sync {
            let beats_per_cycle = LFO_DIVISIONS[self.lfo_division].1;
            tempo / 60.0 / beats_per_cycle
        } else {
            self.lfo_rate
        }
    }
}
