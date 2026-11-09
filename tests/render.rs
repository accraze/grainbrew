//! Offline renders of the DSP engine. Debug builds also check the ring's "no future reads" asserts.

use assert_no_alloc::assert_no_alloc;
use grainbrew::dsp::rng::Rng;
use grainbrew::dsp::{Controls, Engine};
// nih_plug installs the allocator that makes assert_no_alloc abort (debug builds only)
use nih_plug as _;
use std::f32::consts::TAU;

const FS: f32 = 48000.0;
const LEN: usize = 72000;
/// Where signals are steady: after the first grains, before the noise burst ends
const STEADY: std::ops::Range<usize> = 9600..48000;

fn sine(freq: f32) -> Vec<f32> {
    (0..LEN)
        .map(|i| 0.5 * (TAU * freq * i as f32 / FS).sin())
        .collect()
}

fn noise_burst() -> Vec<f32> {
    let mut rng = Rng::new(7);
    (0..LEN)
        .map(|i| if i < 48000 { 0.5 * rng.bipolar() } else { 0.0 })
        .collect()
}

fn render(input: &[f32], controls: Controls) -> Vec<[f32; 2]> {
    let mut engine = Engine::new(FS);
    input
        .iter()
        .map(|&x| engine.process([x, x], &controls))
        .collect()
}

fn rms(x: impl Iterator<Item = f32> + Clone) -> f32 {
    let n = x.clone().count() as f32;
    (x.map(|s| s * s).sum::<f32>() / n).sqrt()
}

/// Output level relative to input over the steady region, in dB.
fn level_db(input: &[f32], output: &[[f32; 2]]) -> f32 {
    let wet = output[STEADY].iter().flatten().copied();
    let dry = input[STEADY].iter().copied();
    20.0 * (rms(wet) / rms(dry)).log10()
}

fn assert_finite(output: &[[f32; 2]]) {
    for (i, s) in output.iter().flatten().enumerate() {
        assert!(s.is_finite(), "sample {i} is {s}");
    }
}

fn signals() -> [Vec<f32>; 2] {
    [sine(440.0), noise_burst()]
}

/// The corners of the control space, where grains are longest, shortest, and most numerous.
#[test]
fn stays_finite_at_extremes() {
    for input in signals() {
        for timbre_semitones in [-24.0, 0.0, 24.0] {
            for spread in [0.0, 1.0] {
                for (density, grain_size_ms) in [(5.0, 2.0), (500.0, 2.0), (500.0, 50.0)] {
                    for (tone, drive) in [(-1.0, 0.0), (1.0, 1.0)] {
                        assert_finite(&render(
                            &input,
                            Controls {
                                timbre_semitones,
                                spread,
                                density,
                                grain_size_ms,
                                tone,
                                drive,
                                ..Controls::default()
                            },
                        ));
                    }
                }
            }
        }
    }
}

#[test]
fn level_is_sane_across_density() {
    for input in signals() {
        for timbre_semitones in [-12.0, 0.0, 12.0] {
            for spread in [0.0, 0.15, 1.0] {
                let levels = [5.0, 20.0, 50.0, 100.0, 200.0, 500.0].map(|density| {
                    level_db(
                        &input,
                        &render(
                            &input,
                            Controls {
                                timbre_semitones,
                                density,
                                spread,
                                ..Controls::default()
                            },
                        ),
                    )
                });
                println!("timbre {timbre_semitones} spread {spread}: {levels:.1?} dB");
                for level in levels {
                    assert!((-24.0..=3.0).contains(&level), "{levels:?}");
                }
            }
        }
    }
}

#[test]
fn process_path_does_not_allocate() {
    for input in signals() {
        let mut engine = Engine::new(FS);
        let controls = Controls {
            timbre_semitones: -24.0,
            spread: 1.0,
            density: 500.0,
            grain_size_ms: 50.0,
            ..Controls::default()
        };
        assert_no_alloc(|| {
            for &x in &input {
                std::hint::black_box(engine.process([x, x], &controls));
            }
            engine.reset();
        });
    }
}

/// Goertzel magnitude of one frequency.
fn magnitude(x: &[f32], freq: f32) -> f32 {
    let k = 2.0 * (TAU * freq / FS).cos();
    let (mut s1, mut s2) = (0.0, 0.0);
    for &sample in x {
        let s = sample + k * s1 - s2;
        s2 = s1;
        s1 = s;
    }
    (s1 * s1 + s2 * s2 - k * s1 * s2).sqrt()
}

#[test]
fn timbre_resamples_grains() {
    let output = render(
        &sine(440.0),
        Controls {
            timbre_semitones: 12.0,
            spread: 0.0,
            ..Controls::default()
        },
    );
    let left: Vec<f32> = output[STEADY].iter().map(|s| s[0]).collect();
    assert!(magnitude(&left, 880.0) > 4.0 * magnitude(&left, 440.0));
}

#[test]
fn spread_colours_the_sound_at_zero_timbre() {
    let input = sine(440.0);
    let difference = |spread| {
        let output = render(
            &input,
            Controls {
                spread,
                ..Controls::default()
            },
        );
        rms(STEADY.map(|i| output[i][0] - input[i]))
    };
    let (plain, spread) = (difference(0.0), difference(0.5));
    assert!(spread > 2.0 * plain, "{plain} vs {spread}");
}

#[test]
fn zero_mix_is_the_dry_signal() {
    let input = noise_burst();
    let output = render(
        &input,
        Controls {
            mix: 0.0,
            ..Controls::default()
        },
    );
    for (dry, out) in input.iter().zip(&output) {
        assert_eq!(*dry, out[0]);
    }
}

#[test]
fn recovers_from_bad_input() {
    let mut engine = Engine::new(FS);
    let controls = Controls::default();
    for x in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        engine.process([x, x], &controls);
    }
    for s in sine(440.0) {
        let out = engine.process([s, s], &controls);
        assert!(out[0].is_finite() && out[1].is_finite());
    }
}

#[test]
fn silence_decays_to_zero_without_denormals() {
    let mut engine = Engine::new(FS);
    let controls = Controls {
        timbre_semitones: -24.0,
        grain_size_ms: 50.0,
        ..Controls::default()
    };
    for s in sine(440.0) {
        engine.process([s, s], &controls);
    }
    let mut last = [1.0; 2];
    for _ in 0..96000 {
        last = engine.process([0.0; 2], &controls);
        assert!(last.iter().all(|s| *s == 0.0 || s.is_normal()));
    }
    assert_eq!(last, [0.0; 2]);
}
