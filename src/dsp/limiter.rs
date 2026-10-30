//! Limiter: when the sound is about to get too loud, turn the volume down a
//! little, smoothly, instead of letting the peaks distort.
//!
//! A clipper bends the *waveform* of loud peaks. On a chord that creates
//! gritty, out-of-tune noise (every note's overtones mix with every other
//! note's). A limiter only changes the *volume*, slowly enough that the
//! waveform keeps its shape, so a big chord just gets a touch quieter.
//!
//! The trick that makes it clean is **lookahead**: the sound is delayed by
//! 1.5 ms, so the limiter sees each peak coming and has already faded the
//! volume down by the time it plays. Afterwards the volume comes back up
//! over 150 ms.

use super::envelope::glide_coef;

/// Peaks are kept at or below this level (about -1.4 dBFS).
const THRESHOLD: f32 = 0.85;
/// How far ahead the limiter looks, in seconds.
const LOOKAHEAD: f32 = 0.0015;
/// How long the volume takes to come back up after a peak (to -60 dB of the
/// way), in seconds.
const RELEASE: f32 = 0.15;
/// Delay buffer size: enough for the lookahead at up to 192 kHz.
const MAX_DELAY: usize = 512;

pub struct Limiter {
    /// The delayed sound, as circular buffers like in `chorus.rs`.
    left: [f32; MAX_DELAY],
    right: [f32; MAX_DELAY],
    position: usize,
    /// Lookahead in samples.
    length: usize,
    /// The volume being applied right now, and the volume it's heading to.
    gain: f32,
    target: f32,
    /// How many more samples to keep `target` low: until the peak that set
    /// it has come out of the delay.
    hold: usize,
    attack_coef: f32,
    release_coef: f32,
}

impl Limiter {
    pub fn new(sample_rate: f32) -> Self {
        let length = ((LOOKAHEAD * sample_rate) as usize).clamp(1, MAX_DELAY);
        Self {
            left: [0.0; MAX_DELAY],
            right: [0.0; MAX_DELAY],
            position: 0,
            length,
            gain: 1.0,
            target: 1.0,
            hold: 0,
            // Fading down covers 95% of the way within the lookahead (3 time
            // constants), so the gain is ready when the peak arrives.
            attack_coef: 1.0 - (-3.0 / length as f32).exp(),
            release_coef: glide_coef(RELEASE, sample_rate),
        }
    }

    /// How many samples late the limiter makes the sound.
    pub fn latency(&self) -> u32 {
        self.length as u32
    }

    pub fn reset(&mut self) {
        self.left = [0.0; MAX_DELAY];
        self.right = [0.0; MAX_DELAY];
        self.gain = 1.0;
        self.target = 1.0;
        self.hold = 0;
    }

    pub fn process(&mut self, left: f32, right: f32) -> (f32, f32) {
        // How much this new sample needs turning down to stay under the
        // threshold (both sides together, so the stereo image doesn't shift).
        let peak = left.abs().max(right.abs());
        let needed = if peak > THRESHOLD {
            THRESHOLD / peak
        } else {
            1.0
        };

        if needed < self.target {
            // A bigger peak is coming: aim lower, and stay there until it
            // has passed through the delay.
            self.target = needed;
            self.hold = self.length;
        } else if self.hold > 0 {
            self.hold -= 1;
        } else {
            self.target = 1.0;
        }

        let coef = if self.target < self.gain {
            self.attack_coef
        } else {
            self.release_coef
        };
        self.gain += (self.target - self.gain) * coef;

        // Swap the new sample into the delay line and take out the one from
        // `length` samples ago.
        let delayed_left = std::mem::replace(&mut self.left[self.position], left);
        let delayed_right = std::mem::replace(&mut self.right[self.position], right);
        self.position = (self.position + 1) % self.length;

        (delayed_left * self.gain, delayed_right * self.gain)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;

    fn sine(n: usize, amplitude: f32) -> f32 {
        amplitude * (std::f32::consts::TAU * 220.0 * n as f32 / SR).sin()
    }

    #[test]
    fn quiet_sound_passes_unchanged_just_later() {
        let mut limiter = Limiter::new(SR);
        let delay = limiter.latency() as usize;
        for n in 0..10_000 {
            let (l, _) = limiter.process(sine(n, 0.5), 0.0);
            if n >= delay {
                assert!((l - sine(n - delay, 0.5)).abs() < 1e-6);
            }
        }
    }

    #[test]
    fn loud_sound_is_turned_down_without_changing_its_shape() {
        let mut limiter = Limiter::new(SR);
        let delay = limiter.latency() as usize;
        for n in 0..(SR as usize) {
            let (l, r) = limiter.process(sine(n, 2.0), sine(n, 2.0));
            assert!(
                l.abs() <= THRESHOLD * 1.06 && r.abs() <= THRESHOLD * 1.06,
                "{l}"
            );
            // Once settled, the output is the same sine, just quieter: no
            // distortion.
            if n > SR as usize / 2 {
                let expected = sine(n - delay, THRESHOLD);
                assert!((l - expected).abs() < 0.01, "{l} vs {expected}");
            }
        }
    }

    #[test]
    fn a_sudden_peak_is_caught_in_advance() {
        let mut limiter = Limiter::new(SR);
        let mut peak: f32 = 0.0;
        for n in 0..10_000 {
            // Silence, then an instant jump to 4x too loud.
            let x = if n < 5_000 { 0.0 } else { sine(n, 3.4) };
            let (l, _) = limiter.process(x, 0.0);
            peak = peak.max(l.abs());
        }
        assert!(peak < 0.92, "peak got through: {peak}");
    }
}
