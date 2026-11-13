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
/// Keeps Freeze a cloud at zero Scatter, instead of every grain replaying the same slice
const FREEZE_MIN_SCATTER_MS: f32 = 50.0;
const BYPASS_FADE_SECONDS: f32 = 0.01;
const SCHEDULER_SEED: u32 = 0x4752_4149;
/// Mean square of a Hann window. N out-of-phase grains sum in power to N * 3/8.
const HANN_POWER: f32 = 0.375;
/// Loop gain at full Feedback. Over 1 so the top of the knob sustains on most settings; how much
/// the loop loses per pass depends on Timbre and Scatter.
const MAX_FEEDBACK_GAIN: f32 = 1.5;

#[derive(Clone, Copy)]
pub struct Controls {
    pub timbre_semitones: f32,
    pub spread: f32,
    /// How much further back grains may start
    pub scatter_ms: f32,
    /// Grains per second
    pub density: f32,
    pub grain_size_ms: f32,
    pub tone: f32,
    pub drive: f32,
    /// 0..1, how much of the shaped cloud is written back into the input buffer
    pub feedback: f32,
    pub mix: f32,
    /// Linear
    pub output_gain: f32,
    /// Stops recording input, so grains keep reading what's held
    pub freeze: bool,
    /// Crossfades to the untouched input. The engine keeps running underneath.
    pub bypass: bool,
}

impl Default for Controls {
    fn default() -> Self {
        Self {
            timbre_semitones: 0.0,
            spread: 0.3,
            scatter_ms: 50.0,
            density: 150.0,
            grain_size_ms: 40.0,
            tone: 0.0,
            drive: 0.0,
            feedback: 0.0,
            mix: 1.0,
            output_gain: 1.0,
            freeze: false,
            bypass: false,
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
    /// Last sample's soft-clipped feedback, written into the ring with the next input
    feedback: [f32; 2],
    /// 1 when the effect is fully in, 0 when bypassed
    engaged: f32,
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
            feedback: [0.0; 2],
            engaged: 1.0,
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
        self.feedback = [0.0; 2];
    }

    pub fn process(&mut self, input: [f32; 2], c: &Controls) -> [f32; 2] {
        // A NaN from the host would otherwise stick in the filters for good
        let dry = input.map(|x| if x.is_finite() { flush(x) } else { 0.0 });

        // Through the highpass, so DC can't build up in the loop
        for (((ring, pre), x), fb) in self
            .rings
            .iter_mut()
            .zip(&mut self.pre)
            .zip(dry)
            .zip(self.feedback)
        {
            let filtered = pre.process(x + fb);
            if !c.freeze {
                ring.push(filtered);
            }
        }

        if self.scheduler.tick(c.density / self.sample_rate) {
            let size_samples = c.grain_size_ms * 0.001 * self.sample_rate;
            let scatter_ms = if c.freeze {
                c.scatter_ms.max(FREEZE_MIN_SCATTER_MS)
            } else {
                c.scatter_ms
            };
            // Real output duration is size / rate, since grains aren't duration-compensated
            let overlap =
                c.density * c.grain_size_ms * 0.001 / 2f32.powf(c.timbre_semitones / 12.0);
            self.grains.spawn(
                self.rings[0].newest(),
                &Spawn {
                    timbre_semitones: c.timbre_semitones,
                    spread: c.spread,
                    size_samples,
                    scatter_samples: scatter_ms * 0.001 * self.sample_rate,
                    frozen: c.freeze,
                    // Floored so sparse grains play at input level instead of being boosted
                    gain: 1.0 / (overlap * HANN_POWER).max(1.0).sqrt(),
                },
            );
        }

        let wet = self.grains.process(&self.rings, c.freeze);

        let target = if c.bypass { 0.0 } else { 1.0 };
        let fade_step = 1.0 / (BYPASS_FADE_SECONDS * self.sample_rate);
        self.engaged += (target - self.engaged).clamp(-fade_step, fade_step);

        let mut out = [0.0; 2];
        for ch in 0..2 {
            let post = &mut self.post[ch];
            post.tilt.set_tone(c.tone);
            post.saturator.set_drive(c.drive);
            let shaped = post.process(wet[ch]);
            // The post saturator already bounds `shaped`, so the loop can't run away. tanh lowers
            // the level it settles at when it sustains.
            self.feedback[ch] = flush((shaped * c.feedback * MAX_FEEDBACK_GAIN).tanh());
            let processed = (dry[ch] + (shaped - dry[ch]) * c.mix) * c.output_gain;
            out[ch] = flush(dry[ch] + (processed - dry[ch]) * self.engaged);
        }
        out
    }
}
