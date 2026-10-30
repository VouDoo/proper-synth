//! Clipping protection: the last line of defense, after the limiter.
//!
//! Digital audio can't go above 1.0 (0 dBFS): anything louder gets chopped
//! flat ("hard clipping"), which sounds like harsh crackling. A *soft* clipper
//! leaves normal levels completely untouched, and only rounds off the peaks
//! that would have gone over, so that they bend smoothly toward the ceiling
//! instead of hitting it.
//!
//! ```text
//!  out
//!  0.98 ┤          ___________   <- ceiling: never exceeded
//!       │       .-´
//!  0.9  ┤     /                  <- threshold: below this, out = in
//!       │   /
//!       │ /
//!     0 ┼──────┬──────────── in
//!             0.9
//! ```

/// Below this level (about -0.9 dBFS) the signal passes through unchanged.
/// The limiter before it keeps peaks at 0.85, so this rarely does anything.
const THRESHOLD: f32 = 0.9;

/// The output gets ever closer to this, but never goes past it.
const CEILING: f32 = 0.98;

pub fn soft_clip(x: f32) -> f32 {
    let size = x.abs();
    if size <= THRESHOLD {
        return x;
    }
    // Above the threshold, tanh bends the excess smoothly toward the ceiling.
    // It starts with a slope of 1, so there's no kink at the threshold.
    let room = CEILING - THRESHOLD;
    let bent = THRESHOLD + room * ((size - THRESHOLD) / room).tanh();
    bent.copysign(x)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quiet_signals_are_untouched() {
        for x in [-0.9, -0.3, 0.0, 0.25, 0.9] {
            assert_eq!(soft_clip(x), x);
        }
    }

    #[test]
    fn loud_signals_stay_under_the_ceiling() {
        let mut previous = soft_clip(0.9);
        for i in 1..1000 {
            let x = 0.9 + i as f32 * 0.05;
            let y = soft_clip(x);
            assert!(y <= CEILING, "{x} -> {y}");
            assert!(y >= previous, "louder input must never get quieter");
            assert_eq!(soft_clip(-x), -y);
            previous = y;
        }
    }
}
