//! The fixed pool of grains.

use super::filters::OnePole;
use super::ring::InputRing;
use super::rng::Rng;
use std::array;
use std::f32::consts::{PI, TAU};

pub const POOL_SIZE: usize = 512;
const WINDOW_SIZE: usize = 1024;
/// Rate deviation at full Spread
const SPREAD_SEMITONES: f32 = 12.0;
/// Oldest-to-newest room the Hermite taps need, plus one sample for rounding
const READ_MARGIN: f32 = 3.0;

#[derive(Clone, Copy)]
struct Grain {
    read_pos: f64,
    rate: f32,
    // f64 because f32 accumulation drifts enough on long grains to outrun READ_MARGIN
    window_phase: f64,
    window_inc: f64,
    /// Per-channel gain, combining level, pan and polarity
    gains: [f32; 2],
    /// Per-grain anti-alias lowpass, which matters when reading faster than 1x
    filter: [OnePole; 2],
    filter_coef: f32,
    rng: Rng,
}

pub struct Spawn {
    pub timbre_semitones: f32,
    pub spread: f32,
    pub size_samples: f32,
    pub gain: f32,
}

pub struct GrainPool {
    grains: Box<[Grain; POOL_SIZE]>,
    /// `grains[..active]` are playing, so the per-sample loop only touches live grains
    active: usize,
    window: Box<[f32; WINDOW_SIZE + 1]>,
}

impl GrainPool {
    pub fn new() -> Self {
        Self {
            grains: Box::new(array::from_fn(|i| Grain {
                read_pos: 0.0,
                rate: 1.0,
                window_phase: 1.0,
                window_inc: 0.0,
                gains: [0.0; 2],
                filter: [OnePole::default(); 2],
                filter_coef: 1.0,
                rng: Rng::new(i as u32),
            })),
            active: 0,
            window: Box::new(array::from_fn(|i| {
                0.5 - 0.5 * (TAU * i as f32 / WINDOW_SIZE as f32).cos()
            })),
        }
    }

    pub fn reset(&mut self) {
        self.active = 0;
    }

    /// Drops the grain if the pool is full. `newest` is the absolute position of the newest input.
    pub fn spawn(&mut self, newest: f64, s: &Spawn) {
        if self.active == POOL_SIZE {
            return;
        }
        let g = &mut self.grains[self.active];
        self.active += 1;

        let semitones = s.timbre_semitones + s.spread * SPREAD_SEMITONES * g.rng.bipolar();
        let rate = 2f32.powf(semitones / 12.0);
        // No duration compensation: the source span is fixed, so a faster grain is a shorter one
        let duration = s.size_samples / rate;

        // A grain faster than 1x gains on the input, so it starts far enough back to never pass it
        g.read_pos = newest - ((rate - 1.0) * duration).max(0.0) as f64 - READ_MARGIN as f64;
        g.rate = rate;
        g.window_phase = 0.0;
        g.window_inc = 1.0 / duration as f64;
        g.filter = [OnePole::default(); 2];
        g.filter_coef = 1.0 - (-PI / rate).exp();

        let pan = s.spread * g.rng.bipolar();
        // bipolar() is uniform, so this flips with probability spread / 2
        let polarity = if g.rng.bipolar() < s.spread - 1.0 {
            -1.0
        } else {
            1.0
        };
        let level = s.gain * polarity;
        g.gains = [level * (1.0 - pan).min(1.0), level * (1.0 + pan).min(1.0)];
    }

    /// Advances every live grain one sample and returns their sum.
    pub fn process(&mut self, rings: &[InputRing; 2]) -> [f32; 2] {
        let mut out = [0.0; 2];
        let mut i = 0;
        while i < self.active {
            if self.grains[i].window_phase >= 1.0 {
                self.active -= 1;
                self.grains.swap(i, self.active);
                continue;
            }
            let g = &mut self.grains[i];
            // Stay in f64: a phase just under 1 rounds to 1 in f32 and would index past the table
            let index = g.window_phase * WINDOW_SIZE as f64;
            let whole = index as usize;
            let w = self.window[whole]
                + (self.window[whole + 1] - self.window[whole]) * index.fract() as f32;

            for c in 0..2 {
                let x = g.filter[c].lowpass(rings[c].read(g.read_pos), g.filter_coef);
                out[c] += x * w * g.gains[c];
            }
            g.read_pos += g.rate as f64;
            g.window_phase += g.window_inc;
            i += 1;
        }
        out
    }
}

impl Default for GrainPool {
    fn default() -> Self {
        Self::new()
    }
}
