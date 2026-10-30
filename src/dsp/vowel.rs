//! Vowel: makes any sound "sing" a vowel, for vocal-chop-like hooks without
//! any samples.
//!
//! What makes an "ah" sound different from an "ee" isn't the voice itself
//! but the shape of the mouth and throat, which boosts a few frequency bands
//! called **formants**. Every vowel has its own set: "ah" has its first two
//! formants close together around 800-1150 Hz, "ee" has them far apart
//! (300 and 2300 Hz).
//!
//! So: run the sound through three narrow bandpass filters, one per formant,
//! tuned to the chosen vowel. Moving the `vowel` setting slides the filters
//! smoothly from one vowel to the next: A → E → I → O → U.

use super::filter::{Coefs, Svf};

/// The vowels, each with three formants: (frequency in Hz, loudness).
/// Classic measurements of a mid-range (tenor-like) voice.
const VOWELS: [(&str, [(f32, f32); 3]); 5] = [
    ("A", [(800.0, 1.0), (1150.0, 0.5), (2900.0, 0.25)]),
    ("E", [(400.0, 1.0), (1600.0, 0.35), (2700.0, 0.2)]),
    ("I", [(300.0, 1.0), (2300.0, 0.3), (3000.0, 0.2)]),
    ("O", [(450.0, 1.0), (800.0, 0.5), (2830.0, 0.1)]),
    ("U", [(325.0, 1.0), (700.0, 0.25), (2530.0, 0.1)]),
];

/// How sharp each formant band is: its frequency divided by its width, so
/// 5 means an 800 Hz band is 160 Hz wide. Sharper sounds more like a voice
/// but low notes then have few harmonics landing inside the bands, which
/// makes the level jump around from note to note; 5 is a good balance.
const SHARPNESS: f32 = 5.0;

/// Narrow bands only let part of the sound through, so boost the result to
/// keep about the same loudness as the unfiltered sound. (Measured: most
/// vowels then land within ±3 dB of the dry sound. "A" on low, dark notes is
/// quieter: it needs energy around 800-1150 Hz that a dark bass lacks.)
const MAKEUP_GAIN: f32 = 3.5;

#[derive(Debug, Clone, Copy, Default)]
pub struct VowelFilter {
    bands: [Svf; 3],
    coefs: [Coefs; 3],
    gains: [f32; 3],
}

impl VowelFilter {
    /// Tune the filters to a vowel. `position` runs from 0.0 (A) to 4.0 (U);
    /// in between, frequencies and loudness blend between the two neighbors.
    pub fn set(&mut self, position: f32, sample_rate: f32) {
        let position = position.clamp(0.0, 4.0);
        let i = (position as usize).min(3);
        let t = position - i as f32;
        for band in 0..3 {
            let (freq_a, gain_a) = VOWELS[i].1[band];
            let (freq_b, gain_b) = VOWELS[i + 1].1[band];
            let freq = freq_a + (freq_b - freq_a) * t;
            self.gains[band] = gain_a + (gain_b - gain_a) * t;
            self.coefs[band] = Coefs::new(freq, SHARPNESS, sample_rate);
        }
    }

    pub fn process(&mut self, input: f32) -> f32 {
        let mut out = 0.0;
        for band in 0..3 {
            let (_, bandpass) = self.bands[band].process(input, &self.coefs[band]);
            out += bandpass * self.gains[band];
        }
        out * MAKEUP_GAIN
    }

    pub fn reset(&mut self) {
        for band in &mut self.bands {
            band.reset();
        }
    }
}

/// A readable name for a vowel position, e.g. "A", "E→I 30%".
pub fn describe(position: f32) -> String {
    let position = position.clamp(0.0, 4.0);
    let i = (position as usize).min(3);
    let t = position - i as f32;
    if t < 0.05 {
        VOWELS[i].0.to_string()
    } else if t > 0.95 {
        VOWELS[i + 1].0.to_string()
    } else {
        format!("{}→{} {:.0}%", VOWELS[i].0, VOWELS[i + 1].0, t * 100.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Level of a sine at `freq` through the vowel filter.
    fn level(position: f32, freq: f32) -> f32 {
        let sr = 96_000.0;
        let mut vowel = VowelFilter::default();
        vowel.set(position, sr);
        let mut peak: f32 = 0.0;
        for n in 0..(sr as usize) {
            let x = (std::f32::consts::TAU * freq * n as f32 / sr).sin();
            let y = vowel.process(x);
            if n > sr as usize / 2 {
                peak = peak.max(y.abs());
            }
        }
        peak
    }

    #[test]
    fn each_vowel_boosts_its_own_formants() {
        // "A" is strong around 800 Hz, "I" isn't; "I" is strong at 2300 Hz.
        assert!(level(0.0, 800.0) > 4.0 * level(2.0, 800.0));
        assert!(level(2.0, 2300.0) > 2.0 * level(0.0, 2300.0));
    }

    #[test]
    fn names_read_well() {
        assert_eq!(describe(0.0), "A");
        assert_eq!(describe(1.3), "E→I 30%");
        assert_eq!(describe(4.0), "U");
    }
}
