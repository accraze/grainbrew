//! xorshift32: small, allocation-free randomness for the audio thread.

#[derive(Clone, Copy)]
pub struct Rng(u32);

impl Rng {
    pub fn new(seed: u32) -> Self {
        // Scramble and warm up so neighbouring seeds don't start out correlated. Zero is xorshift's
        // one stuck state, and `| 1` keeps us out of it.
        let mut rng = Self(seed.wrapping_add(1).wrapping_mul(0x9E37_79B9) | 1);
        for _ in 0..8 {
            rng.next_u32();
        }
        rng
    }

    pub fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }

    /// Uniform in -1..1.
    pub fn bipolar(&mut self) -> f32 {
        self.next_u32() as f32 / 2_147_483_648.0 - 1.0
    }
}
