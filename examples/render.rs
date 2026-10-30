//! Render a 2-operator FM tone to a WAV file: the simplest possible FM
//! synth, and a good first thing to read before `src/dsp/`.
//!
//! Usage:
//!   cargo run --example render -- [freq] [ratio] [index] [decay] [out.wav]
//!
//! Defaults: 220 Hz, ratio 1, index 3, decay 1.5 s, writes `fm.wav`.
//! `decay` is how fast the index and volume fade (seconds to ~37%);
//! use 0 for no decay.
//!
//! Things to try:
//!   index 0            -> pure sine
//!   ratio 1, index 1-5 -> sawtooth-ish, gets brighter as index rises
//!   ratio 2            -> hollow, square-ish (odd harmonics)
//!   ratio 3.5, index 6 -> bell
//!   ratio 14, index 2  -> metallic / electric piano tine

use proper_synth::dsp::operator::{Operator, phase_increment};
use std::f32::consts::TAU;

const SAMPLE_RATE: u32 = 48_000;
const SECONDS: f32 = 3.0;

/// One modulator feeding one carrier.
///
/// ```text
///   modulator (freq * ratio) ──× index──▶ carrier (freq) ──▶ out
/// ```
struct TwoOpVoice {
    freq: f32,
    /// Modulator frequency as a multiple of the carrier frequency.
    /// Whole numbers (1, 2, 3...) sound harmonic; others sound bell-like.
    ratio: f32,
    /// Modulation index (in radians): how strongly the modulator bends the
    /// carrier's phase. 0 = pure sine, higher = brighter / more sidebands.
    index: f32,
    carrier: Operator,
    modulator: Operator,
}

impl TwoOpVoice {
    fn next_sample(&mut self, sample_rate: f32) -> f32 {
        let m = self
            .modulator
            .next_sample(phase_increment(self.freq * self.ratio, sample_rate), 0.0);
        // `index` is in radians, the operator wants cycles: divide by 2π.
        self.carrier.next_sample(
            phase_increment(self.freq, sample_rate),
            self.index * m / TAU,
        )
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let arg = |i: usize, default: f32| -> f32 {
        args.get(i).and_then(|s| s.parse().ok()).unwrap_or(default)
    };

    let freq = arg(0, 220.0);
    let ratio = arg(1, 1.0);
    let index = arg(2, 3.0);
    let decay = arg(3, 1.5);
    let path = args.get(4).map(String::as_str).unwrap_or("fm.wav");

    let mut voice = TwoOpVoice {
        freq,
        ratio,
        index,
        carrier: Operator::new(),
        modulator: Operator::new(),
    };

    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: SAMPLE_RATE,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(path, spec)?;

    let sr = SAMPLE_RATE as f32;
    let total = (SECONDS * sr) as usize;

    for n in 0..total {
        let t = n as f32 / sr;

        // A crude envelope: exponential decay, plus a few ms fade-in/out so the
        // start and end don't click. Proper ADSR envelopes come later.
        let env = if decay > 0.0 { (-t / decay).exp() } else { 1.0 };
        let fade = (t / 0.005).min(1.0).min((SECONDS - t) / 0.005);

        // Decaying the index too is the classic FM trick: the sound starts
        // bright and mellows out, like a real struck/plucked instrument.
        voice.index = index * env;

        let sample = voice.next_sample(sr) * env * fade * 0.5;
        writer.write_sample(sample)?;
    }

    writer.finalize()?;
    println!("wrote {path}: {freq} Hz, ratio {ratio}, index {index}, decay {decay}s");
    Ok(())
}
