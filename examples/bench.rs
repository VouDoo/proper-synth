//! Rough CPU benchmark: how long it takes to render 10 seconds of audio with
//! 32 voices playing, for every preset.
//!
//! Usage: cargo run --release --example bench

use proper_synth::dsp::Synth;
use proper_synth::presets::PRESETS;
use std::time::Instant;

const SAMPLE_RATE: f32 = 48_000.0;
const SECONDS: f32 = 10.0;
/// The plugin reads its knobs every 32 samples, then renders one sample at
/// a time; this does the same, so the numbers match real use.
const BLOCK: usize = 32;

fn main() {
    for (name, patch) in PRESETS {
        // Sustain every note so all 32 voices stay busy the whole time.
        let mut patch = *patch;
        for op in &mut patch.ops {
            op.sustain = op.sustain.max(0.5);
        }

        let mut synth = Synth::new(SAMPLE_RATE, &patch);
        for note in 0..32 {
            synth.note_on(36 + note, 0.8);
        }

        let blocks = (SECONDS * SAMPLE_RATE) as usize / BLOCK;
        let (mut left, mut right) = ([0.0; BLOCK], [0.0; BLOCK]);
        let mut sum = 0.0;
        let start = Instant::now();
        for _ in 0..blocks {
            synth.set_patch(&patch);
            for i in 0..BLOCK {
                (left[i], right[i]) = synth.next_sample();
            }
            sum += left.iter().sum::<f32>() + right.iter().sum::<f32>();
        }
        let elapsed = start.elapsed().as_secs_f32();

        // "CPU %" = share of one core needed to keep up in real time.
        println!(
            "{name:12} {:5.1}% of one core (32 voices)   [{sum:.0}]",
            elapsed / SECONDS * 100.0
        );
    }
}
