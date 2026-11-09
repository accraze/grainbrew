//! One-pole filters. State is flushed by hand, so the DSP stays denormal-free without CPU flags
//! (offline tests have none; inside a host nih-plug also enables FTZ).

use std::f32::consts::TAU;

const DENORMAL_FLOOR: f32 = 1e-20;
/// Tilt gain at either end of the Tone knob
const TILT_DB: f32 = 6.0;
const TILT_PIVOT_HZ: f32 = 1000.0;

pub fn flush(x: f32) -> f32 {
    if x.abs() < DENORMAL_FLOOR {
        0.0
    } else {
        x
    }
}

pub fn one_pole_coef(hz: f32, sample_rate: f32) -> f32 {
    1.0 - (-TAU * hz / sample_rate).exp()
}

#[derive(Clone, Copy, Default)]
pub struct OnePole(f32);

impl OnePole {
    pub fn lowpass(&mut self, x: f32, coef: f32) -> f32 {
        self.0 = flush(self.0 + coef * (x - self.0));
        self.0
    }
}

#[derive(Clone, Copy)]
pub struct Highpass {
    low: OnePole,
    coef: f32,
}

impl Highpass {
    pub fn new(hz: f32, sample_rate: f32) -> Self {
        Self {
            low: OnePole::default(),
            coef: one_pole_coef(hz, sample_rate),
        }
    }

    pub fn process(&mut self, x: f32) -> f32 {
        x - self.low.lowpass(x, self.coef)
    }
}

/// Tilts around a pivot: one side comes up as the other goes down.
#[derive(Clone, Copy)]
pub struct Tilt {
    low: OnePole,
    coef: f32,
    low_gain: f32,
    high_gain: f32,
}

impl Tilt {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            low: OnePole::default(),
            coef: one_pole_coef(TILT_PIVOT_HZ, sample_rate),
            low_gain: 1.0,
            high_gain: 1.0,
        }
    }

    /// `tone` is -1 (dark) to 1 (bright).
    pub fn set_tone(&mut self, tone: f32) {
        self.high_gain = 10f32.powf(tone * TILT_DB / 20.0);
        self.low_gain = 1.0 / self.high_gain;
    }

    pub fn process(&mut self, x: f32) -> f32 {
        let low = self.low.lowpass(x, self.coef);
        low * self.low_gain + (x - low) * self.high_gain
    }
}
