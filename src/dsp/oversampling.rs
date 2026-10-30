//! Oversampling: computing the sound at twice the sample rate, then filtering
//! and keeping every other sample.
//!
//! Why: a digital signal can't contain frequencies above half its sample rate
//! (the "Nyquist" frequency, 24 kHz at 48 kHz). FM easily creates overtones
//! higher than that, and instead of disappearing they *fold back* down into
//! the audible range as harsh, out-of-tune noise. That's called aliasing.
//!
//! At 96 kHz there's room up to 48 kHz. Everything between 20 and 48 kHz is
//! then removed by a lowpass filter, *before* we drop back to 48 kHz, so it
//! never gets the chance to fold back.

use std::f64::consts::PI;

/// Filter length. More taps = steeper filter (passes more of the treble and
/// blocks more of the aliasing), but costs more CPU and adds delay.
const TAPS: usize = 97;

/// Takes two samples at the doubled rate, returns one at the normal rate.
pub struct Downsampler2x {
    coefs: [f32; TAPS],
    /// The last `TAPS` input samples, oldest first.
    history: [f32; TAPS],
}

impl Default for Downsampler2x {
    fn default() -> Self {
        Self {
            coefs: lowpass_coefs(),
            history: [0.0; TAPS],
        }
    }
}

impl Downsampler2x {
    /// How many output samples late the filter makes the sound (the filter
    /// "looks" at samples around its center, so the center is the delay).
    pub const LATENCY: u32 = ((TAPS - 1) / 2 / 2) as u32;

    pub fn process(&mut self, a: f32, b: f32) -> f32 {
        // Drop the two oldest samples, append the two new ones.
        self.history.copy_within(2.., 0);
        self.history[TAPS - 2] = a;
        self.history[TAPS - 1] = b;

        // The filter: a weighted sum of recent samples (a "FIR" filter).
        let mut out = 0.0;
        for (coef, sample) in self.coefs.iter().zip(&self.history) {
            out += coef * sample;
        }
        out
    }

    pub fn reset(&mut self) {
        self.history = [0.0; TAPS];
    }
}

/// A "windowed sinc" lowpass at a quarter of the (doubled) sample rate, i.e.
/// at the normal rate's Nyquist frequency.
///
/// The sinc function is the shape of a perfect lowpass filter, but it goes on
/// forever. A *window* tapers it to a finite length; the Kaiser window does
/// that very cleanly, giving a flat response up to ~21 kHz and over 85 dB of
/// attenuation from ~27 kHz.
fn lowpass_coefs() -> [f32; TAPS] {
    // Kaiser's beta trades steepness for attenuation; 9 gives about -90 dB.
    const BETA: f64 = 9.0;

    let center = (TAPS - 1) as f64 / 2.0;
    let mut coefs = [0.0f64; TAPS];
    for (i, coef) in coefs.iter_mut().enumerate() {
        let x = (i as f64 - center) * PI / 2.0;
        let sinc = if x == 0.0 { 1.0 } else { x.sin() / x };
        // Position across the filter from -1 to 1.
        let t = (i as f64 - center) / center;
        let kaiser = bessel_i0(BETA * (1.0 - t * t).sqrt()) / bessel_i0(BETA);
        *coef = sinc * kaiser;
    }
    // Scale so the coefficients add up to 1: low frequencies pass unchanged.
    let sum: f64 = coefs.iter().sum();
    coefs.map(|c| (c / sum) as f32)
}

/// The modified Bessel function I0, which the Kaiser window is built from,
/// computed from its power series.
fn bessel_i0(x: f64) -> f64 {
    let mut sum = 1.0;
    let mut term = 1.0;
    for k in 1..50 {
        term *= (x / 2.0 / k as f64).powi(2);
        sum += term;
    }
    sum
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Feed a sine at `freq` (in Hz, at a 96 kHz input rate) and return the
    /// output level in dB. Measures RMS rather than peak, because at high
    /// frequencies the samples rarely land exactly on the peaks.
    fn gain_db(freq: f64) -> f64 {
        let mut ds = Downsampler2x::default();
        let input = |k: usize| (std::f64::consts::TAU * freq * k as f64 / 96_000.0).sin() as f32;
        let mut sum_of_squares = 0.0;
        let mut count = 0;
        for n in 0..8000 {
            let out = ds.process(input(2 * n), input(2 * n + 1)) as f64;
            if n > TAPS {
                sum_of_squares += out * out;
                count += 1;
            }
        }
        let rms = (sum_of_squares / count as f64).sqrt();
        // A sine's RMS is its peak / √2.
        20.0 * (rms * 2f64.sqrt()).log10()
    }

    #[test]
    fn passes_the_audible_range() {
        for freq in [100.0, 1_000.0, 10_000.0, 18_000.0, 20_000.0] {
            assert!(gain_db(freq).abs() < 0.1, "{freq} Hz: {} dB", gain_db(freq));
        }
    }

    #[test]
    fn blocks_what_would_alias() {
        for freq in [28_000.0, 35_000.0, 45_000.0] {
            assert!(gain_db(freq) < -85.0, "{freq} Hz: {} dB", gain_db(freq));
        }
    }
}
