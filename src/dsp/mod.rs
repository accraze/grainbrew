//! Granular engine, free of plugin-framework code so it can be rendered offline.

pub mod filters;
pub mod grains;
pub mod ring;
pub mod rng;
pub mod saturator;
pub mod scheduler;

use filters::{flush, Highpass, Tilt};
use grains::{GrainPool, Spawn};
use ring::InputRing;
use saturator::Saturator;
use scheduler::Scheduler;

const PRE_HIGHPASS_HZ: f32 = 40.0;
const SCHEDULER_SEED: u32 = 0x4752_4149;
/// A Hann window averages 0.5, so N overlapping in-phase grains sum to N/2
const HANN_MEAN: f32 = 0.5;

#[derive(Clone, Copy)]
pub struct Controls {
    pub timbre_semitones: f32,
    pub spread: f32,
    /// Grains per second
    pub density: f32,
    pub grain_size_ms: f32,
    pub tone: f32,
    pub drive: f32,
    pub mix: f32,
    /// Linear
    pub output_gain: f32,
}

impl Default for Controls {
    fn default() -> Self {
        Self {
            timbre_semitones: 0.0,
            spread: 0.15,
            density: 100.0,
            grain_size_ms: 20.0,
            tone: 0.0,
            drive: 0.0,
            mix: 1.0,
            output_gain: 1.0,
        }
    }
}

/// Post stage. Swap the two calls in `process` to reorder.
#[derive(Clone, Copy)]
struct Post {
    tilt: Tilt,
    saturator: Saturator,
}

impl Post {
    fn process(&mut self, x: f32) -> f32 {
        self.saturator.process(self.tilt.process(x))
    }
}

pub struct Engine {
    sample_rate: f32,
    pre: [Highpass; 2],
    rings: [InputRing; 2],
    scheduler: Scheduler,
    grains: GrainPool,
    post: [Post; 2],
}

impl Engine {
    /// Allocates everything the engine will ever use, so call it off the audio thread.
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            pre: [Highpass::new(PRE_HIGHPASS_HZ, sample_rate); 2],
            rings: [InputRing::new(sample_rate), InputRing::new(sample_rate)],
            scheduler: Scheduler::new(SCHEDULER_SEED),
            grains: GrainPool::new(),
            post: [Post {
                tilt: Tilt::new(sample_rate),
                saturator: Saturator::default(),
            }; 2],
        }
    }

    /// Clears playback state, keeping allocations.
    pub fn reset(&mut self) {
        let sample_rate = self.sample_rate;
        self.pre = [Highpass::new(PRE_HIGHPASS_HZ, sample_rate); 2];
        self.rings.iter_mut().for_each(InputRing::reset);
        self.scheduler.reset();
        self.grains.reset();
        self.post = [Post {
            tilt: Tilt::new(sample_rate),
            saturator: Saturator::default(),
        }; 2];
    }

    pub fn process(&mut self, input: [f32; 2], c: &Controls) -> [f32; 2] {
        // A NaN from the host would otherwise stick in the filters for good
        let dry = input.map(|x| if x.is_finite() { flush(x) } else { 0.0 });

        for ((ring, pre), x) in self.rings.iter_mut().zip(&mut self.pre).zip(dry) {
            ring.push(pre.process(x));
        }

        if self.scheduler.tick(c.density / self.sample_rate) {
            let size_samples = c.grain_size_ms * 0.001 * self.sample_rate;
            // Real output duration is size / rate, since grains aren't duration-compensated
            let overlap =
                c.density * c.grain_size_ms * 0.001 / 2f32.powf(c.timbre_semitones / 12.0);
            self.grains.spawn(
                self.rings[0].newest(),
                &Spawn {
                    timbre_semitones: c.timbre_semitones,
                    spread: c.spread,
                    size_samples,
                    // Floored so sparse grains play at input level instead of being boosted
                    gain: 1.0 / (overlap * HANN_MEAN).max(1.0),
                },
            );
        }

        let wet = self.grains.process(&self.rings);

        let mut out = [0.0; 2];
        for ch in 0..2 {
            let post = &mut self.post[ch];
            post.tilt.set_tone(c.tone);
            post.saturator.set_drive(c.drive);
            let shaped = post.process(wet[ch]);
            out[ch] = flush((dry[ch] + (shaped - dry[ch]) * c.mix) * c.output_gain);
        }
        out
    }
}
