//! Decides on which samples grains start.

use super::rng::Rng;

/// Each interval is 1 +- this fraction of the nominal one
const JITTER: f32 = 0.5;

pub struct Scheduler {
    phase: f32,
    next: f32,
    rng: Rng,
}

impl Scheduler {
    pub fn new(seed: u32) -> Self {
        Self {
            phase: 0.0,
            next: 0.0,
            rng: Rng::new(seed),
        }
    }

    pub fn reset(&mut self) {
        self.phase = 0.0;
        self.next = 0.0;
    }

    /// Call once per sample; true means start a grain. An accumulator rather than a sample
    /// countdown, so a density change takes effect now and not at the next spawn.
    pub fn tick(&mut self, grains_per_sample: f32) -> bool {
        self.phase += grains_per_sample;
        if self.phase < self.next {
            return false;
        }
        self.phase -= self.next;
        self.next = 1.0 + JITTER * self.rng.bipolar();
        true
    }
}
