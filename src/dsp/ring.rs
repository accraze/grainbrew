//! Input history that grains read from.

pub struct InputRing {
    buf: Vec<f32>,
    mask: usize,
    written: u64,
}

impl InputRing {
    /// Rounds up to a power of two so reads can wrap with a mask (1.37 s at 48 kHz).
    pub fn new(sample_rate: f32) -> Self {
        let len = (sample_rate.ceil() as usize).next_power_of_two();
        Self {
            buf: vec![0.0; len],
            mask: len - 1,
            written: 0,
        }
    }

    pub fn reset(&mut self) {
        self.buf.fill(0.0);
        self.written = 0;
    }

    pub fn push(&mut self, x: f32) {
        self.buf[self.written as usize & self.mask] = x;
        self.written += 1;
    }

    /// Absolute position of the newest sample. Grains keep absolute positions so they never wrap.
    pub fn newest(&self) -> f64 {
        self.written as f64 - 1.0
    }

    /// Cubic Hermite read at an absolute position.
    pub fn read(&self, pos: f64) -> f32 {
        let floor = pos.floor();
        // Grain scheduling must keep all four taps inside the history (zero latency, no stale data)
        debug_assert!(floor + 2.0 <= self.newest(), "read from the future");
        debug_assert!(
            floor - 1.0 > self.newest() - self.buf.len() as f64,
            "read past the oldest sample"
        );

        let t = (pos - floor) as f32;
        // Positions start negative after a reset. The cast wraps, landing on still-silent samples.
        let at = |offset: i64| self.buf[(floor as i64 + offset) as usize & self.mask];
        let (xm1, x0, x1, x2) = (at(-1), at(0), at(1), at(2));

        let c = (x1 - xm1) * 0.5;
        let v = x0 - x1;
        let w = c + v;
        let a = w + v + (x2 - x0) * 0.5;
        let b = w + a;
        ((a * t - b) * t + c) * t + x0
    }
}
