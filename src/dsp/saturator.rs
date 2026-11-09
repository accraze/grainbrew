//! Soft clipper.

/// Input gain at full Drive
const MAX_DRIVE_GAIN: f32 = 9.0;

#[derive(Clone, Copy)]
pub struct Saturator {
    gain: f32,
    makeup: f32,
}

impl Default for Saturator {
    fn default() -> Self {
        Self {
            gain: 1.0,
            makeup: 1.0,
        }
    }
}

impl Saturator {
    /// `drive` is 0..1. Zero is unity for small signals and still clips softly near full scale.
    pub fn set_drive(&mut self, drive: f32) {
        self.gain = 1.0 + (MAX_DRIVE_GAIN - 1.0) * drive;
        self.makeup = 1.0 / self.gain.sqrt();
    }

    pub fn process(&mut self, x: f32) -> f32 {
        (x * self.gain).tanh() * self.makeup
    }
}
