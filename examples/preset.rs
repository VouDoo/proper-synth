//! Render a preset to a stereo WAV file, using the same engine as the plugin:
//! a chord and a melody, or a bassline for mono presets.
//!
//! Usage:
//!   cargo run --release --example preset -- "Warm Keys" [out.wav]
//!   cargo run --release --example preset -- list

use nih_plug::util;
use proper_synth::dsp::Synth;
use proper_synth::params::DEFAULT_GAIN_DB;
use proper_synth::presets::{PRESETS, find};

const SAMPLE_RATE: u32 = 48_000;

/// (start time in seconds, length in seconds, MIDI note, velocity 0..1)
const NOTES: &[(f32, f32, u8, f32)] = &[
    // A minor 9 chord, a typical deep house voicing.
    (0.0, 1.5, 45, 0.8),
    (0.0, 1.5, 57, 0.7),
    (0.0, 1.5, 60, 0.7),
    (0.0, 1.5, 64, 0.7),
    (0.0, 1.5, 71, 0.7),
    // A little phrase, soft to hard, to hear velocity at work.
    (2.0, 0.2, 69, 0.4),
    (2.25, 0.2, 72, 0.6),
    (2.5, 0.2, 76, 0.8),
    (2.75, 0.6, 81, 1.0),
];

/// A garage bassline in A minor at 125 BPM. Notes that start before the
/// previous one ends overlap, so mono presets slide between them.
const BASSLINE: &[(f32, f32, u8, f32)] = &[
    (0.00, 0.20, 33, 0.9),
    (0.36, 0.10, 33, 0.7),
    (0.48, 0.30, 45, 0.8),
    (0.72, 0.30, 43, 0.8), // overlaps: slides down from A2 to G2
    (0.96, 0.20, 40, 0.9), // and on to E2
    (1.44, 0.20, 36, 0.9),
    (1.80, 0.40, 38, 0.8),
    (2.10, 0.50, 33, 0.9), // slides from D2 down to A1
];

const SECONDS: f32 = 4.5;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let name = args.first().map(String::as_str).unwrap_or("list");

    let Some(patch) = find(name) else {
        println!("Presets:");
        for (name, _) in PRESETS {
            println!("  {name}");
        }
        return Ok(());
    };
    let default_path = format!("{}.wav", name.to_lowercase().replace(' ', "_"));
    let path = args.get(1).cloned().unwrap_or(default_path);

    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: SAMPLE_RATE,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(&path, spec)?;

    let sr = SAMPLE_RATE as f32;
    let mut synth = Synth::new(sr, &patch);
    synth.set_gain(util::db_to_gain(DEFAULT_GAIN_DB));
    synth.reset();
    let mut peak: f32 = 0.0;

    let notes = if patch.mono { BASSLINE } else { NOTES };

    for n in 0..(SECONDS * sr) as usize {
        // Send note-ons and note-offs that land exactly on this sample.
        for &(start, length, note, velocity) in notes {
            if n == (start * sr) as usize {
                synth.note_on(note, velocity);
            }
            if n == ((start + length) * sr) as usize {
                synth.note_off(note);
            }
        }

        let (left, right) = synth.next_sample();
        peak = peak.max(left.abs()).max(right.abs());
        writer.write_sample(left)?;
        writer.write_sample(right)?;
    }

    writer.finalize()?;
    println!("wrote {path} (peak {peak:.2})");
    Ok(())
}
