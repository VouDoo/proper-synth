use std::sync::LazyLock;

/// The phase is a 32-bit integer where the full range is one cycle:
/// 0 = start, 2^31 = half way, and adding past 2^32 - 1 wraps back to 0 for
/// free, exactly like a cycle repeats. (The DX7 did the same.) This is both
/// faster and more precise than a float that we'd have to wrap ourselves.
const CYCLE: f32 = 4_294_967_296.0; // 2^32

/// The table index is the phase's top 12 bits: 4096 entries.
const TABLE_BITS: u32 = 12;
const TABLE_SIZE: usize = 1 << TABLE_BITS;

/// One cycle of a sine wave, computed once at startup.
///
/// Reading it (and blending between the two nearest entries) is cheaper than
/// calling `sin()`, and with 4096 entries the error is below 1e-6, around
/// -130 dB: far below anything audible. The extra entry at the end is a copy
/// of the first, so blending never reads past the end.
pub(crate) static SINE_TABLE: LazyLock<[f32; TABLE_SIZE + 1]> = LazyLock::new(|| {
    std::array::from_fn(|i| (std::f64::consts::TAU * i as f64 / TABLE_SIZE as f64).sin() as f32)
});

/// How much the phase advances per sample to play `freq` Hz.
pub fn phase_increment(freq: f32, sample_rate: f32) -> u32 {
    // Through u64 so frequencies above the sample rate wrap instead of
    // getting stuck at the maximum.
    (freq as f64 / sample_rate as f64 * CYCLE as f64) as u64 as u32
}

/// Convert a phase offset in cycles (any size, positive or negative) into
/// phase units. Like the phase itself, it wraps: 1.25 cycles = 0.25 cycles.
fn cycles_to_phase(cycles: f32) -> u32 {
    (cycles * CYCLE) as i64 as u32
}

/// sin(2π · phase / 2^32).
pub fn sine(phase: u32) -> f32 {
    let i = (phase >> (32 - TABLE_BITS)) as usize;
    // The remaining low bits say how far we are between entry i and i + 1.
    let frac = (phase << TABLE_BITS) as f32 / CYCLE;
    let table = &*SINE_TABLE;
    table[i] + (table[i + 1] - table[i]) * frac
}

/// An FM "operator": a sine oscillator whose phase can be pushed around by
/// other operators.
#[derive(Debug, Clone, Default)]
pub struct Operator {
    phase: u32,
}

impl Operator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Advance one sample and return the output (-1..1).
    ///
    /// `increment` comes from `phase_increment`. `phase_mod` (in cycles) is
    /// added to the phase *before* looking up the sine. With `phase_mod = 0.0`
    /// this is a plain sine wave; feeding another operator's output in here is
    /// what makes it FM.
    #[inline(always)]
    pub fn next_sample(&mut self, increment: u32, phase_mod: f32) -> f32 {
        let out = sine(self.phase.wrapping_add(cycles_to_phase(phase_mod)));
        self.phase = self.phase.wrapping_add(increment);
        out
    }

    pub fn reset(&mut self) {
        self.phase = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::TAU;

    #[test]
    fn sine_table_matches_sin() {
        for i in 0..10_000 {
            let cycles = i as f32 / 10_000.0;
            let error = (sine(cycles_to_phase(cycles)) - (TAU * cycles).sin()).abs();
            assert!(error < 1e-5, "{cycles} cycles: error {error}");
        }
    }

    #[test]
    fn phase_offsets_wrap_like_cycles() {
        for cycles in [-2.75f32, -0.25, 0.75, 3.75] {
            let error = (sine(cycles_to_phase(cycles)) - (TAU * cycles).sin()).abs();
            assert!(error < 1e-4, "{cycles} cycles: error {error}");
        }
    }

    #[test]
    fn plays_the_right_frequency() {
        // 1000 Hz at 48 kHz = one cycle every 48 samples, so 48 samples later
        // we're back where we started.
        let mut op = Operator::new();
        let increment = phase_increment(1000.0, 48_000.0);
        let first = op.next_sample(increment, 0.25);
        for _ in 0..47 {
            op.next_sample(increment, 0.25);
        }
        assert!((op.next_sample(increment, 0.25) - first).abs() < 1e-4);
    }
}
