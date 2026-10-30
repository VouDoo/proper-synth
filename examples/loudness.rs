//! Measure how loud each preset is when played normally (basses as one low
//! note, everything else as a chord), to keep presets equally loud.
//!
//! Usage: cargo run --release --example loudness
//!
//! Loudness = the loudest 0.3 s stretch in the first 2 s, which works for
//! short hits and slow pads alike. If you change a preset, adjust its
//! `volume_db` by the difference from the others.

use nih_plug::util;
use proper_synth::dsp::Synth;
use proper_synth::params::DEFAULT_GAIN_DB;
use proper_synth::presets::PRESETS;
fn main() {
    let sr = 48_000.0;
    let window = 14_400; // 0.3 s
    for (name, patch) in PRESETS {
        let notes: &[u8] = if name.contains("Bass") {
            &[36]
        } else {
            &[57, 60, 64, 67]
        };
        let mut synth = Synth::new(sr, patch);
        synth.set_gain(util::db_to_gain(DEFAULT_GAIN_DB));
        synth.reset();
        for &n in notes {
            synth.note_on(n, 0.8);
        }
        // Loudness of the mono sum: what a club's (often mono) system plays.
        let samples: Vec<f32> = (0..96_000)
            .map(|_| {
                let (l, r) = synth.next_sample();
                (l + r) / 2.0
            })
            .collect();
        // Loudest 0.3 s stretch in the first 2 s, stepping by 0.05 s.
        let loudest = (0..samples.len() - window)
            .step_by(2_400)
            .map(|start| {
                let sum: f64 = samples[start..start + window]
                    .iter()
                    .map(|&s| (s as f64).powi(2))
                    .sum();
                10.0 * (sum / window as f64).log10()
            })
            .fold(f64::MIN, f64::max);
        println!("{name:18} {loudest:6.1}");
    }
}
