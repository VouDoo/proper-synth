//! A resonant filter: the classic way to shape a synth's tone over time.
//!
//! A *lowpass* filter lets frequencies below its **cutoff** through and
//! darkens everything above it. Sweeping the cutoff up and down is the
//! "wah" of a stab opening up or a bass talking. **Resonance** boosts a narrow
//! peak right at the cutoff, which makes sweeps squelchy.
//!
//! This is a *state-variable filter* in the "TPT" form (from Andrew Simper's
//! papers), the go-to design in modern plugins: it stays stable and in tune
//! even when the cutoff moves every single sample. As a bonus, the same
//! calculation also gives a *bandpass* output (only a band around the cutoff
//! passes), which `vowel.rs` uses.

use std::f32::consts::PI;

/// The filter's settings, turned into the numbers `Svf::process` needs.
/// Building these costs a division, so do it only when the cutoff changes.
#[derive(Debug, Clone, Copy, Default)]
pub struct Coefs {
    k: f32,
    a1: f32,
    a2: f32,
    a3: f32,
}

impl Coefs {
    /// `q` is resonance as a "quality" factor: 0.7 is a plain smooth
    /// lowpass, higher values make an ever sharper peak at the cutoff.
    pub fn new(cutoff: f32, q: f32, sample_rate: f32) -> Self {
        let g = fast_tan(PI * cutoff / sample_rate);
        let k = 1.0 / q;
        let a1 = 1.0 / (1.0 + g * (g + k));
        Self {
            k,
            a1,
            a2: g * a1,
            a3: g * g * a1,
        }
    }
}

/// The filter's memory: two values that carry over from sample to sample.
/// (In the analog circuits this design models, they're two capacitors.)
#[derive(Debug, Clone, Copy, Default)]
pub struct Svf {
    s1: f32,
    s2: f32,
}

impl Svf {
    /// Filter one sample. Returns (lowpass, bandpass); the bandpass is scaled
    /// so its peak has a gain of 1.
    pub fn process(&mut self, input: f32, c: &Coefs) -> (f32, f32) {
        let v3 = input - self.s2;
        let band = c.a1 * self.s1 + c.a2 * v3;
        let low = self.s2 + c.a2 * self.s1 + c.a3 * v3;
        self.s1 = 2.0 * band - self.s1;
        self.s2 = 2.0 * low - self.s2;
        (low, band * c.k)
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// tan(x), within 0.002% up to x = 1 and 0.1% up to x = 1.4 (cutoffs up to
/// 45% of the sample rate). In practice the cutoff stays under 20 kHz at a
/// doubled rate of 88 kHz or more, so x < 0.72 and the error is ~0.0002%.
/// Several times cheaper than `f32::tan`, which matters because a moving
/// cutoff needs a new value every sample.
fn fast_tan(x: f32) -> f32 {
    let x2 = x * x;
    x * (105.0 - 10.0 * x2) / (105.0 - 45.0 * x2 + x2 * x2)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 96_000.0;

    /// Level in dB of a sine at `freq` after the lowpass.
    fn lowpass_db(freq: f32, cutoff: f32, q: f32) -> f32 {
        let coefs = Coefs::new(cutoff, q, SR);
        let mut filter = Svf::default();
        let mut peak: f32 = 0.0;
        for n in 0..(SR as usize) {
            let x = (std::f32::consts::TAU * freq * n as f32 / SR).sin();
            let (low, _) = filter.process(x, &coefs);
            if n > SR as usize / 2 {
                peak = peak.max(low.abs());
            }
        }
        20.0 * peak.log10()
    }

    #[test]
    fn fast_tan_is_accurate() {
        for i in 1..=140 {
            let x = i as f32 / 100.0;
            let error = (fast_tan(x) - x.tan()).abs() / x.tan();
            let allowed = if x <= 1.0 { 2e-5 } else { 1e-3 };
            assert!(error < allowed, "x = {x}: error {error}");
        }
    }

    #[test]
    fn lowpass_passes_lows_and_cuts_highs() {
        assert!(lowpass_db(100.0, 1_000.0, 0.7).abs() < 0.5);
        // 12 dB per octave: 3 octaves above the cutoff is about -36 dB.
        assert!(lowpass_db(8_000.0, 1_000.0, 0.7) < -30.0);
    }

    #[test]
    fn resonance_makes_a_peak_at_the_cutoff() {
        assert!(lowpass_db(1_000.0, 1_000.0, 8.0) > 15.0);
    }

    #[test]
    fn stays_stable_when_swept_hard() {
        let mut filter = Svf::default();
        for n in 0..(SR as usize) {
            // Sweep 20 Hz to 20 kHz forty times a second, at full resonance.
            let sweep = (n as f32 * 40.0 / SR).fract();
            let coefs = Coefs::new(20.0 * 1000f32.powf(sweep), 16.0, SR);
            let (low, band) = filter.process(if n % 100 < 50 { 1.0 } else { -1.0 }, &coefs);
            assert!(low.is_finite() && low.abs() < 100.0, "blew up: {low}");
            assert!(band.is_finite());
        }
    }
}
