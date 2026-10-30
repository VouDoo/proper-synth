//! Stereo chorus: what makes a single synth sound wide, like several slightly
//! different players at once.
//!
//! It mixes the sound with delayed copies of itself. The delay time keeps
//! swinging slowly back and forth, which bends the copies' pitch a tiny bit
//! up and down. Left and right swing in opposite directions, so the two sides
//! differ and the sound spreads across the stereo field.
//!
//! ```text
//!          ┌──────────────────────────────┐
//!  in ─────┤                              ├─ dry ─┬──▶ left
//!          └─▶ delay line ─┬─ tap (7 ms ± 2) ─────┘
//!                          └─ tap (7 ms ∓ 2) ─────┬──▶ right
//!                                          dry ───┘
//! ```

use super::operator::{Operator, phase_increment};

/// Delay line length, in samples: room for the longest delay at 192 kHz.
const MAX_DELAY: usize = 8192;
/// Average delay of the copies.
const DELAY: f32 = 0.007;
/// How far the delay swings either way.
const DEPTH: f32 = 0.002;
/// How fast it swings, in Hz.
const RATE: f32 = 0.6;

pub struct Chorus {
    /// The last `MAX_DELAY` input samples, used as a circular buffer:
    /// `write` moves forward and wraps around, overwriting the oldest sample.
    buffer: [f32; MAX_DELAY],
    write: usize,
    lfo: Operator,
    lfo_increment: u32,
    sample_rate: f32,
}

impl Chorus {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            buffer: [0.0; MAX_DELAY],
            write: 0,
            lfo: Operator::new(),
            lfo_increment: phase_increment(RATE, sample_rate),
            sample_rate,
        }
    }

    pub fn reset(&mut self) {
        self.buffer = [0.0; MAX_DELAY];
    }

    /// Process one sample. `amount` 0..1 goes from dry to a 50/50 mix.
    /// Returns (left, right).
    pub fn process(&mut self, input: f32, amount: f32) -> (f32, f32) {
        self.buffer[self.write] = input;

        let lfo = self.lfo.next_sample(self.lfo_increment, 0.0);
        let left = self.read(DELAY + DEPTH * lfo);
        let right = self.read(DELAY - DEPTH * lfo);

        self.write = (self.write + 1) % MAX_DELAY;

        let dry = 1.0 - 0.5 * amount;
        let wet = 0.5 * amount;
        (input * dry + left * wet, input * dry + right * wet)
    }

    /// Read the input from `seconds` ago. The delay rarely lands exactly on
    /// a sample, so blend the two nearest ones.
    fn read(&self, seconds: f32) -> f32 {
        let position = self.write as f32 - seconds * self.sample_rate + MAX_DELAY as f32;
        let i = position as usize;
        let frac = position - i as f32;
        let a = self.buffer[i % MAX_DELAY];
        let b = self.buffer[(i + 1) % MAX_DELAY];
        a + (b - a) * frac
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_amount_passes_the_sound_through() {
        let mut chorus = Chorus::new(48_000.0);
        for n in 0..1000 {
            let x = (n as f32 * 0.1).sin();
            assert_eq!(chorus.process(x, 0.0), (x, x));
        }
    }

    #[test]
    fn makes_left_and_right_differ() {
        let mut chorus = Chorus::new(48_000.0);
        let mut difference: f32 = 0.0;
        for n in 0..48_000 {
            let (l, r) = chorus.process((n as f32 * 0.05).sin(), 1.0);
            difference = difference.max((l - r).abs());
        }
        assert!(difference > 0.1, "not stereo: {difference}");
    }
}
