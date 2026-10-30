//! Everything that makes sound. Nothing in here knows about plugins, MIDI
//! devices or audio drivers: it's plain Rust that turns notes into samples.
//!
//! Suggested reading order:
//! 1. `operator`     - a sine oscillator
//! 2. `envelope`     - shapes loudness over time
//! 3. `filter`       - shaping the tone (and the "wah")
//! 4. `vowel`        - making a sound sing a vowel
//! 5. `algorithm`    - how operators modulate each other
//! 6. `patch`        - the settings that make up one sound
//! 7. `voice`        - one playing note, putting 1-6 together
//! 8. `polyphony`    - many voices at once
//! 9. `oversampling` - keeping high overtones from turning into noise
//! 10. `chorus`      - turning one voice into a wide stereo sound
//! 11. `limiter`     - turning loud moments down cleanly
//! 12. `clipper`     - the last safety net against overloads
//! 13. `synth`       - the finished instrument

pub mod algorithm;
pub mod chorus;
pub mod clipper;
pub mod envelope;
pub mod filter;
pub mod limiter;
pub mod operator;
pub mod oversampling;
pub mod patch;
pub mod polyphony;
pub mod synth;
pub mod voice;
pub mod vowel;

pub use patch::{FILTER_OPEN, FilterPatch, LFO_DIVISIONS, NUM_OPS, OpPatch, Patch};
pub use synth::Synth;
