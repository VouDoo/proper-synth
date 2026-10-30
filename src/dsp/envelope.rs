/// Below this level we consider an envelope silent (-80 dB).
const SILENCE: f32 = 1e-4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Stage {
    #[default]
    Idle,
    Attack,
    /// Decay and sustain share a stage: the level glides toward the sustain
    /// level and simply stays there while the key is held.
    DecaySustain,
    Release,
}

/// ADSR envelope: a number between 0 and 1 that changes over the life of a
/// note. Multiply an operator's output by it to shape the note.
///
/// ```text
///  1 ┤  /\
///    │ /  \___________  <- sustain level
///    │/               \
///  0 ┼─A──D─────S──────R──
///       key down     key up
/// ```
///
/// Attack is a straight line up. Decay and release are exponential curves,
/// because that's how real sounds fade (and it sounds smoother to our ears).
/// Their times are how long the level takes to fall by 60 dB, which is about
/// when you stop hearing it: "release 0.5 s" really means half a second.
///
/// Turning times into per-sample steps needs `exp()`, which is slow, so that
/// happens in `set` (once per block of samples) and `next_sample` (every
/// sample) only does a multiply and an add.
#[derive(Debug, Clone, Default)]
pub struct Adsr {
    stage: Stage,
    level: f32,
    attack_step: f32,
    decay_coef: f32,
    sustain: f32,
    release_coef: f32,
}

impl Adsr {
    /// Set the shape. Times are in seconds, `sustain` is a level 0..1.
    pub fn set(&mut self, attack: f32, decay: f32, sustain: f32, release: f32, sr: f32) {
        self.attack_step = 1.0 / (attack * sr);
        self.decay_coef = glide_coef(decay, sr);
        self.sustain = sustain;
        self.release_coef = glide_coef(release, sr);
    }

    pub fn note_on(&mut self) {
        // We keep the current level instead of jumping to 0: if this voice is
        // still sounding, a jump would make an audible click.
        self.stage = Stage::Attack;
    }

    pub fn note_off(&mut self) {
        if self.stage != Stage::Idle {
            self.stage = Stage::Release;
        }
    }

    pub fn is_active(&self) -> bool {
        self.stage != Stage::Idle
    }

    pub fn reset(&mut self) {
        self.stage = Stage::Idle;
        self.level = 0.0;
    }

    /// Advance one sample.
    pub fn next_sample(&mut self) -> f32 {
        match self.stage {
            Stage::Idle => {}
            Stage::Attack => {
                self.level += self.attack_step;
                if self.level >= 1.0 {
                    self.level = 1.0;
                    self.stage = Stage::DecaySustain;
                }
            }
            Stage::DecaySustain => {
                self.level += (self.sustain - self.level) * self.decay_coef;
                // With sustain at 0 the note dies by itself, like a piano or
                // a pluck, even while the key is still held.
                if self.sustain < SILENCE && self.level < SILENCE {
                    self.reset();
                }
            }
            Stage::Release => {
                self.level -= self.level * self.release_coef;
                if self.level < SILENCE {
                    self.reset();
                }
            }
        }
        self.level
    }
}

/// How far to move toward the target each sample, so that after `time`
/// seconds 99.9% of the distance is covered (a drop of 60 dB). Repeating this
/// every sample traces an exponential curve. A time of 0 gives 1: jump
/// straight to the target. Also used for glide, gain changes and the limiter.
pub(crate) fn glide_coef(time: f32, sr: f32) -> f32 {
    // ln(1000): the number of "e-fold" steps in a 1000x (60 dB) drop.
    const LN_1000: f32 = 6.907_755;
    1.0 - (-LN_1000 / (time * sr)).exp()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 1000.0;

    fn run(env: &mut Adsr, seconds: f32) -> f32 {
        let mut level = 0.0;
        for _ in 0..(seconds * SR) as usize {
            level = env.next_sample();
        }
        level
    }

    #[test]
    fn goes_through_all_stages() {
        let mut env = Adsr::default();
        env.set(0.01, 0.1, 0.5, 0.1, SR);
        env.note_on();
        assert!((run(&mut env, 0.01) - 1.0).abs() < 1e-3, "attack reaches 1");
        assert!(
            (run(&mut env, 2.0) - 0.5).abs() < 1e-3,
            "settles at sustain"
        );
        env.note_off();
        // 0.1 s release = -60 dB after 0.1 s.
        let level = run(&mut env, 0.1);
        assert!(
            (level - 0.5e-3).abs() < 0.1e-3,
            "release time is -60 dB: {level}"
        );
        env.note_on();
        run(&mut env, 2.0);
        env.note_off();
        run(&mut env, 2.0);
        assert!(!env.is_active(), "release ends the note");
    }

    #[test]
    fn zero_sustain_ends_by_itself() {
        let mut env = Adsr::default();
        env.set(0.001, 0.1, 0.0, 0.1, SR);
        env.note_on();
        run(&mut env, 2.0);
        assert!(!env.is_active());
    }
}
