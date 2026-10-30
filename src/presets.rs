//! Factory sounds, voiced for UK garage and house: organs, stabs, keys,
//! hooks, pads and basses. Each row is one operator:
//!
//! ```text
//!   op(ratio, level, velocity, attack, decay, sustain, release)
//! ```
//!
//! Times are in seconds; decay and release are the time to fade by 60 dB.
//!
//! Algorithms are numbered as in `dsp/algorithm.rs` (1-8). A good way to learn
//! is to copy a preset, change one number, and listen.

use crate::dsp::{FILTER_OPEN, FilterPatch, OpPatch, Patch};

const fn op(
    ratio: f32,
    level: f32,
    velocity: f32,
    attack: f32,
    decay: f32,
    sustain: f32,
    release: f32,
) -> OpPatch {
    OpPatch {
        ratio,
        level,
        velocity,
        attack,
        decay,
        sustain,
        release,
    }
}

/// The starting point for every preset: no mono, LFO, filter, vowel or
/// chorus. Each preset below sets what it needs and ends with `..BASE` for
/// everything else.
const BASE: Patch = Patch {
    algorithm: 1,
    feedback: 0.0,
    key_track: 0.0,
    volume_db: 0.0,
    mono: false,
    glide: 0.0,
    lfo_rate: 5.0,
    lfo_sync: false,
    lfo_division: 6, // 1/8
    lfo_pitch: 0.0,
    lfo_wobble: 0.0,
    lfo_volume: 0.0,
    lfo_cutoff: 0.0,
    filter: FILTER_OPEN,
    vowel: 0.0,
    vowel_mix: 0.0,
    chorus: 0.0,
    ops: [op(1.0, 0.0, 0.0, 0.001, 0.1, 0.0, 0.1); 4],
};

pub const PRESETS: &[(&str, Patch)] = &[
    (
        // The speed garage organ: bright and hollow (fundamental, octave and
        // 3rd harmonic, like organ drawbars), with op 4 adding a short click
        // to every hit. Settles to a lower level so chords don't wash out.
        "Garage Organ",
        Patch {
            algorithm: 7,
            key_track: 0.2,
            volume_db: 5.1,
            chorus: 0.3,
            ops: [
                op(1.0, 1.00, 0.3, 0.001, 1.2, 0.60, 0.08),
                op(2.0, 0.80, 0.3, 0.001, 0.9, 0.45, 0.08),
                op(3.0, 0.50, 0.5, 0.001, 0.5, 0.20, 0.06),
                op(2.0, 0.25, 0.7, 0.001, 0.06, 0.00, 0.05),
            ],
            ..BASE
        },
    ),
    (
        // Soulful house organ: four sine "drawbars" (sub-octave, fundamental,
        // octave, and the 3rd harmonic an octave up) that sustain fully. The
        // LFO adds a light vibrato and tremolo, and the chorus spreads it out,
        // together imitating a spinning rotary speaker.
        "Gospel Organ",
        Patch {
            algorithm: 8,
            volume_db: 5.6,
            lfo_rate: 6.0,
            lfo_pitch: 0.08,
            lfo_volume: 0.1,
            chorus: 0.7,
            ops: [
                op(0.5, 0.70, 0.2, 0.005, 0.5, 1.00, 0.05),
                op(1.0, 1.00, 0.2, 0.005, 0.5, 1.00, 0.05),
                op(2.0, 0.60, 0.2, 0.005, 0.5, 1.00, 0.05),
                op(3.0, 0.40, 0.2, 0.005, 0.5, 1.00, 0.05),
            ],
            ..BASE
        },
    ),
    (
        // Short, punchy chord hit for swung 2-step patterns. Feedback on op 4
        // makes it buzzy like a saw wave; everything dies in under half a
        // second so the groove stays tight.
        "2-Step Stab",
        Patch {
            algorithm: 2,
            feedback: 0.35,
            key_track: 0.2,
            volume_db: 7.3,
            chorus: 0.35,
            ops: [
                op(1.0, 1.00, 0.5, 0.001, 0.45, 0.00, 0.10),
                op(1.0, 0.32, 0.7, 0.001, 0.3, 0.00, 0.10),
                op(2.0, 0.15, 0.6, 0.001, 0.18, 0.00, 0.08),
                op(1.0, 0.12, 0.6, 0.001, 0.15, 0.00, 0.08),
            ],
            ..BASE
        },
    ),
    (
        // Warm, slightly detuned saw-like chord stab, the core sound of 2010s
        // garage-revival house. Heavy feedback makes a bright saw, then the
        // filter darkens it and its envelope opens it for a moment on every
        // hit: the analog-style stab "bow". Wide chorus to finish.
        "Revival Stab",
        Patch {
            algorithm: 5,
            feedback: 0.8,
            key_track: 0.2,
            volume_db: 4.1,
            filter: FilterPatch {
                cutoff: 700.0,
                resonance: 0.25,
                env_amount: 0.45,
                attack: 0.001,
                decay: 0.35,
                sustain: 0.15,
                release: 0.2,
                key_track: 0.5,
                velocity: 0.5,
            },
            chorus: 0.55,
            ops: [
                op(1.0, 1.00, 0.4, 0.002, 0.8, 0.15, 0.20),
                op(1.0, 0.30, 0.5, 0.001, 0.8, 0.30, 0.20),
                op(1.006, 0.90, 0.4, 0.002, 0.8, 0.15, 0.20),
                op(1.0, 0.30, 0.4, 0.001, 0.8, 0.30, 0.20),
            ],
            ..BASE
        },
    ),
    (
        // Chicago house brass stab: one long chain with feedback for a brassy
        // buzz. The modulators open a moment after the note starts, like a
        // player blowing harder, then the hit falls back quickly.
        "House Brass",
        Patch {
            algorithm: 1,
            feedback: 0.6,
            key_track: 0.1,
            volume_db: 3.8,
            chorus: 0.3,
            ops: [
                op(1.0, 1.00, 0.4, 0.01, 0.6, 0.20, 0.12),
                op(1.0, 0.35, 0.7, 0.03, 0.5, 0.30, 0.10),
                op(1.0, 0.15, 0.5, 0.02, 0.4, 0.30, 0.10),
                op(1.0, 0.12, 0.5, 0.02, 0.4, 0.30, 0.10),
            ],
            ..BASE
        },
    ),
    (
        // Punchy garage piano for chord stabs: an octave layer (op 3) for
        // brightness and a ratio 5 "hammer" (op 4) that only lasts a moment.
        "Garage Keys",
        Patch {
            algorithm: 5,
            key_track: 0.5,
            volume_db: 5.2,
            chorus: 0.2,
            ops: [
                op(1.0, 1.00, 0.5, 0.001, 1.5, 0.00, 0.15),
                op(1.0, 0.40, 0.8, 0.001, 0.5, 0.00, 0.12),
                op(2.0, 0.45, 0.5, 0.001, 1.0, 0.00, 0.15),
                op(5.0, 0.12, 0.9, 0.001, 0.15, 0.00, 0.10),
            ],
            ..BASE
        },
    ),
    (
        // Deep-house electric piano. Ops 2→1 are the warm body, op 4 at ratio
        // 14 gives op 3 a short metallic "tine" on each hit. Op 3 sits a hair
        // sharp (1.002), so the two halves beat slowly against each other. A
        // gentle tremolo and chorus, like the classic electric piano sound.
        "Warm Keys",
        Patch {
            algorithm: 5,
            key_track: 0.4,
            volume_db: 2.6,
            lfo_rate: 4.5,
            lfo_volume: 0.2,
            chorus: 0.4,
            ops: [
                op(1.0, 1.00, 0.6, 0.002, 6.0, 0.15, 0.50),
                op(1.0, 0.18, 0.9, 0.001, 2.5, 0.05, 0.50),
                op(1.002, 0.50, 0.7, 0.001, 4.0, 0.00, 0.40),
                op(14.0, 0.12, 1.0, 0.001, 0.5, 0.00, 0.30),
            ],
            ..BASE
        },
    ),
    (
        // Garage bell hook. The ratio 3.5 and 7 modulators give a bright,
        // slightly metallic attack that rings out quickly over a clean tone.
        "Bell Lead",
        Patch {
            algorithm: 5,
            key_track: 0.5,
            volume_db: 6.3,
            chorus: 0.25,
            ops: [
                op(1.0, 1.00, 0.5, 0.001, 1.6, 0.00, 0.40),
                op(3.5, 0.18, 0.8, 0.001, 0.6, 0.00, 0.30),
                op(2.0, 0.35, 0.6, 0.001, 0.8, 0.00, 0.30),
                op(7.0, 0.10, 0.9, 0.001, 0.2, 0.00, 0.20),
            ],
            ..BASE
        },
    ),
    (
        // Bright pluck for riffs and hooks: a quick snap (op 2) over a short
        // tail, with a touch of feedback grit.
        "Pluck Lead",
        Patch {
            algorithm: 3,
            feedback: 0.2,
            key_track: 0.3,
            volume_db: 4.6,
            chorus: 0.3,
            ops: [
                op(1.0, 1.00, 0.5, 0.001, 0.7, 0.00, 0.20),
                op(1.0, 0.30, 0.8, 0.001, 0.2, 0.00, 0.15),
                op(3.0, 0.12, 0.6, 0.001, 0.1, 0.00, 0.10),
                op(1.0, 0.12, 0.5, 0.001, 0.1, 0.00, 0.10),
            ],
            ..BASE
        },
    ),
    (
        // Very short pluck for fast 16th-note arpeggios: a resonant filter
        // "blip" opens and snaps shut on every note, and everything is gone
        // within a third of a second so quick patterns stay clear.
        "Arp Pluck",
        Patch {
            algorithm: 3,
            feedback: 0.5,
            key_track: 0.4,
            volume_db: 8.5,
            filter: FilterPatch {
                cutoff: 600.0,
                resonance: 0.35,
                env_amount: 0.55,
                attack: 0.001,
                decay: 0.2,
                sustain: 0.0,
                release: 0.1,
                key_track: 0.6,
                velocity: 0.5,
            },
            chorus: 0.4,
            ops: [
                op(1.0, 1.00, 0.5, 0.001, 0.35, 0.00, 0.12),
                op(1.0, 0.35, 0.8, 0.001, 0.3, 0.00, 0.10),
                op(2.0, 0.08, 0.6, 0.001, 0.08, 0.00, 0.08),
                op(1.0, 0.20, 0.5, 0.001, 0.3, 0.00, 0.06),
            ],
            ..BASE
        },
    ),
    (
        // Vocal-chop-style hook: a bright, saw-like pluck sung through the
        // "A" vowel, so short notes sound like chopped "ah"s. Try moving
        // Vowel toward E or O for other syllables.
        "Vox Chop",
        Patch {
            algorithm: 3,
            feedback: 0.7,
            key_track: 0.3,
            volume_db: 1.7,
            vowel: 0.0,
            vowel_mix: 0.9,
            chorus: 0.3,
            ops: [
                op(1.0, 1.00, 0.5, 0.002, 0.6, 0.25, 0.15),
                op(1.0, 0.30, 0.6, 0.001, 0.5, 0.30, 0.15),
                op(2.0, 0.10, 0.5, 0.001, 0.3, 0.20, 0.10),
                op(1.0, 0.25, 0.5, 0.001, 0.5, 0.40, 0.15),
            ],
            ..BASE
        },
    ),
    (
        // Slow, warm pad. The modulators fade in slower than the carriers, so
        // the tone opens up gradually; op 3 is detuned against op 1 for a
        // gentle beating, and a wide chorus surrounds it.
        "Felt Pad",
        Patch {
            algorithm: 5,
            key_track: 0.2,
            volume_db: 0.5,
            chorus: 0.6,
            ops: [
                op(1.0, 1.00, 0.3, 0.4, 3.0, 0.80, 2.50),
                op(1.0, 0.10, 0.3, 0.8, 4.0, 0.50, 2.50),
                op(1.004, 0.90, 0.3, 0.5, 3.0, 0.80, 2.50),
                op(2.0, 0.06, 0.3, 1.0, 4.0, 0.60, 2.50),
            ],
            ..BASE
        },
    ),
    (
        // Bright, wide pad for emotional breakdowns. Feedback gives the second
        // pair a saw-like shine, the filter slowly opens as the pad fades in,
        // op 3 is detuned for movement, and a very slow vibrato makes the
        // whole chord drift gently.
        "Lush Pad",
        Patch {
            algorithm: 5,
            feedback: 0.5,
            key_track: 0.2,
            volume_db: 1.2,
            lfo_rate: 0.3,
            lfo_pitch: 0.04,
            filter: FilterPatch {
                cutoff: 1500.0,
                resonance: 0.15,
                env_amount: 0.25,
                attack: 0.6,
                decay: 3.0,
                sustain: 0.6,
                release: 1.5,
                key_track: 0.3,
                velocity: 0.0,
            },
            chorus: 0.8,
            ops: [
                op(1.0, 1.00, 0.3, 0.25, 3.0, 0.85, 1.50),
                op(1.0, 0.20, 0.3, 0.5, 4.0, 0.70, 1.50),
                op(1.007, 0.90, 0.3, 0.3, 3.0, 0.85, 1.50),
                op(1.0, 0.20, 0.3, 0.4, 4.0, 0.70, 1.50),
            ],
            ..BASE
        },
    ),
    (
        // The classic house "aah" choir pad: two detuned saw-like pairs sung
        // through the A vowel, with a slow fade-in, gentle vibrato and a wide
        // chorus, like a section of singers.
        "Choir Pad",
        Patch {
            algorithm: 5,
            feedback: 0.6,
            key_track: 0.2,
            volume_db: -1.5,
            lfo_rate: 5.0,
            lfo_pitch: 0.05,
            vowel: 0.0,
            vowel_mix: 0.85,
            chorus: 0.8,
            ops: [
                op(1.0, 1.00, 0.2, 0.3, 3.0, 0.90, 1.50),
                op(1.0, 0.20, 0.2, 0.4, 3.0, 0.80, 1.50),
                op(1.005, 0.90, 0.2, 0.3, 3.0, 0.90, 1.50),
                op(1.0, 0.20, 0.2, 0.4, 3.0, 0.80, 1.50),
            ],
            ..BASE
        },
    ),
    (
        // Gritty speed garage bass. Op 4's feedback makes a saw-like buzz, op 3
        // an octave down adds growl, and op 2 gives every note a pluck. Mono:
        // overlap two notes to slide between them.
        "Speed Bass",
        Patch {
            algorithm: 3,
            feedback: 0.6,
            volume_db: 1.8,
            mono: true,
            glide: 0.05,
            ops: [
                op(1.0, 1.00, 0.3, 0.002, 1.0, 0.75, 0.08),
                op(1.0, 0.28, 0.6, 0.001, 0.2, 0.12, 0.08),
                op(0.5, 0.18, 0.4, 0.001, 1.0, 0.60, 0.08),
                op(1.0, 0.20, 0.4, 0.001, 0.5, 0.50, 0.08),
            ],
            ..BASE
        },
    ),
    (
        // The speed garage wobble: a resonant filter opens and closes on every
        // eighth note, in time with the track, so the bass talks. The FM
        // source is a bright, sustained saw so the filter has plenty to shape.
        "Wobble Bass",
        Patch {
            algorithm: 3,
            feedback: 0.7,
            volume_db: -0.1,
            mono: true,
            glide: 0.05,
            lfo_sync: true,
            lfo_division: 6, // 1/8
            lfo_cutoff: 0.6,
            filter: FilterPatch {
                cutoff: 400.0,
                resonance: 0.45,
                env_amount: 0.2,
                attack: 0.001,
                decay: 0.3,
                sustain: 0.5,
                release: 0.1,
                key_track: 0.3,
                velocity: 0.3,
            },
            ops: [
                op(1.0, 1.00, 0.3, 0.002, 1.0, 0.80, 0.08),
                op(1.0, 0.35, 0.5, 0.001, 0.3, 0.30, 0.08),
                op(0.5, 0.20, 0.4, 0.001, 1.0, 0.60, 0.08),
                op(1.0, 0.30, 0.4, 0.001, 0.5, 0.60, 0.08),
            ],
            ..BASE
        },
    ),
    (
        // Hollow organ bass for hypnotic repeating riffs. A ratio 2 modulator
        // gives a square-wave-like hollowness, op 3 adds a sub octave, and op 4
        // a quick click on every hit.
        "Organ Riff Bass",
        Patch {
            algorithm: 5,
            volume_db: 9.6,
            mono: true,
            glide: 0.03,
            ops: [
                op(1.0, 1.00, 0.4, 0.001, 0.5, 0.30, 0.06),
                op(2.0, 0.30, 0.6, 0.001, 0.25, 0.10, 0.06),
                op(0.5, 0.55, 0.3, 0.002, 0.8, 0.50, 0.07),
                op(3.0, 0.10, 0.7, 0.001, 0.05, 0.00, 0.05),
            ],
            ..BASE
        },
    ),
    (
        // Gritty, growling bass. Heavy feedback on op 4 makes a raw saw-like
        // buzz; the LFO, synced to quarter notes, moves both the FM grit and a
        // resonant filter, so long notes growl in time with the track.
        "Growl Bass",
        Patch {
            algorithm: 3,
            feedback: 0.85,
            volume_db: 0.9,
            mono: true,
            glide: 0.03,
            lfo_sync: true,
            lfo_division: 3, // 1/4
            lfo_wobble: 0.3,
            lfo_cutoff: 0.3,
            filter: FilterPatch {
                cutoff: 600.0,
                resonance: 0.3,
                env_amount: 0.3,
                attack: 0.001,
                decay: 0.4,
                sustain: 0.4,
                release: 0.1,
                key_track: 0.3,
                velocity: 0.3,
            },
            ops: [
                op(1.0, 1.00, 0.3, 0.002, 1.0, 0.80, 0.07),
                op(1.0, 0.25, 0.5, 0.001, 0.3, 0.20, 0.07),
                op(0.5, 0.25, 0.4, 0.001, 1.0, 0.70, 0.07),
                op(1.0, 0.30, 0.4, 0.001, 0.6, 0.60, 0.07),
            ],
            ..BASE
        },
    ),
    (
        // Classic FM house bass: a plucky, rubbery top (ops 2→1) layered with
        // a clean sine an octave down (op 3) for weight. Op 4 is unused.
        "Pluck Bass",
        Patch {
            algorithm: 5,
            volume_db: 8.0,
            mono: true,
            glide: 0.04,
            ops: [
                op(1.0, 1.00, 0.4, 0.001, 0.8, 0.40, 0.07),
                op(1.0, 0.35, 0.8, 0.001, 0.18, 0.06, 0.07),
                op(0.5, 0.70, 0.3, 0.002, 1.0, 0.70, 0.08),
                op(1.0, 0.00, 0.0, 0.001, 0.1, 0.00, 0.10),
            ],
            ..BASE
        },
    ),
    (
        // Clean sub: nearly a pure sine, with a short burst of modulation on
        // the attack so it still "knocks" on small speakers. A slower glide for
        // those sliding garage sub lines. Ops 3-4 are unused.
        "Sub Bass",
        Patch {
            algorithm: 1,
            volume_db: 0.6,
            mono: true,
            glide: 0.08,
            ops: [
                op(1.0, 1.00, 0.3, 0.003, 2.0, 0.90, 0.12),
                op(1.0, 0.10, 0.8, 0.001, 0.12, 0.02, 0.10),
                op(1.0, 0.00, 0.0, 0.001, 0.1, 0.00, 0.10),
                op(1.0, 0.00, 0.0, 0.001, 0.1, 0.00, 0.10),
            ],
            ..BASE
        },
    ),
];

/// Find a preset by name, ignoring upper/lower case.
pub fn find(name: &str) -> Option<Patch> {
    PRESETS
        .iter()
        .find(|(preset_name, _)| preset_name.eq_ignore_ascii_case(name))
        .map(|(_, patch)| *patch)
}
