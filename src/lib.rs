use dsp::{Controls, Engine};
use nih_plug::prelude::*;
use nih_plug_egui::EguiState;
use std::num::NonZeroU32;
use std::sync::Arc;

pub mod dsp;
mod editor;

struct Grainbrew {
    params: Arc<GrainbrewParams>,
    engine: Engine,
}

// Timbre, Spread, Scatter, Density and Grain Size are unsmoothed: they're only read when a grain starts, and
// each grain keeps its own settings, so a step in them can't click.
#[derive(Params)]
pub struct GrainbrewParams {
    #[persist = "editor-state"]
    editor_state: Arc<EguiState>,
    #[id = "timbre"]
    timbre: FloatParam,
    #[id = "spread"]
    spread: FloatParam,
    #[id = "scatter"]
    scatter: FloatParam,
    #[id = "density"]
    density: FloatParam,
    #[id = "size"]
    grain_size: FloatParam,
    #[id = "tone"]
    tone: FloatParam,
    #[id = "drive"]
    drive: FloatParam,
    #[id = "mix"]
    mix: FloatParam,
    #[id = "output"]
    output: FloatParam,
    #[id = "freeze"]
    freeze: BoolParam,
    #[id = "bypass"]
    bypass: BoolParam,
}

fn percent(name: &str, default: f32) -> FloatParam {
    FloatParam::new(name, default, FloatRange::Linear { min: 0.0, max: 1.0 })
        .with_unit("%")
        .with_value_to_string(formatters::v2s_f32_percentage(0))
        .with_string_to_value(formatters::s2v_f32_percentage())
}

impl Default for GrainbrewParams {
    fn default() -> Self {
        let defaults = Controls::default();
        Self {
            editor_state: EguiState::from_size(editor::WIDTH, editor::HEIGHT),
            timbre: FloatParam::new(
                "Timbre",
                defaults.timbre_semitones,
                FloatRange::Linear {
                    min: -24.0,
                    max: 24.0,
                },
            )
            .with_unit(" st")
            .with_value_to_string(formatters::v2s_f32_rounded(1)),
            spread: percent("Spread", defaults.spread),
            scatter: FloatParam::new(
                "Scatter",
                defaults.scatter_ms,
                FloatRange::Skewed {
                    min: 0.0,
                    max: 250.0,
                    factor: FloatRange::skew_factor(-1.0),
                },
            )
            .with_unit(" ms")
            .with_value_to_string(formatters::v2s_f32_rounded(0)),
            density: FloatParam::new(
                "Density",
                defaults.density,
                FloatRange::Skewed {
                    min: 5.0,
                    max: 500.0,
                    factor: FloatRange::skew_factor(-2.0),
                },
            )
            .with_unit(" /s")
            .with_value_to_string(formatters::v2s_f32_rounded(0)),
            grain_size: FloatParam::new(
                "Size",
                defaults.grain_size_ms,
                FloatRange::Skewed {
                    min: 2.0,
                    max: 50.0,
                    factor: FloatRange::skew_factor(-1.0),
                },
            )
            .with_unit(" ms")
            .with_value_to_string(formatters::v2s_f32_rounded(1)),
            tone: FloatParam::new(
                "Tone",
                defaults.tone,
                FloatRange::Linear {
                    min: -1.0,
                    max: 1.0,
                },
            )
            .with_smoother(SmoothingStyle::Linear(20.0))
            .with_value_to_string(formatters::v2s_f32_rounded(2)),
            drive: percent("Drive", defaults.drive).with_smoother(SmoothingStyle::Linear(20.0)),
            mix: percent("Mix", defaults.mix).with_smoother(SmoothingStyle::Linear(20.0)),
            output: FloatParam::new(
                "Output",
                defaults.output_gain,
                FloatRange::Skewed {
                    min: util::db_to_gain(-24.0),
                    max: util::db_to_gain(12.0),
                    factor: FloatRange::gain_skew_factor(-24.0, 12.0),
                },
            )
            .with_smoother(SmoothingStyle::Logarithmic(50.0))
            .with_unit(" dB")
            .with_value_to_string(formatters::v2s_f32_gain_to_db(1))
            .with_string_to_value(formatters::s2v_f32_gain_to_db()),
            freeze: BoolParam::new("Freeze", defaults.freeze),
            bypass: BoolParam::new("Bypass", defaults.bypass).make_bypass(),
        }
    }
}

impl Default for Grainbrew {
    fn default() -> Self {
        Self {
            params: Arc::new(GrainbrewParams::default()),
            engine: Engine::new(48000.0),
        }
    }
}

impl Plugin for Grainbrew {
    const NAME: &'static str = "Grainbrew";
    const VENDOR: &'static str = "accraze";
    const URL: &'static str = "";
    const EMAIL: &'static str = "";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(2),
            main_output_channels: NonZeroU32::new(2),
            ..AudioIOLayout::const_default()
        },
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(1),
            main_output_channels: NonZeroU32::new(1),
            ..AudioIOLayout::const_default()
        },
    ];

    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        editor::create(self.params.clone())
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        context: &mut impl InitContext<Self>,
    ) -> bool {
        // Allocated here because reset() can run on the audio thread
        self.engine = Engine::new(buffer_config.sample_rate);
        context.set_latency_samples(0);
        true
    }

    fn reset(&mut self) {
        self.engine.reset();
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let p = &self.params;
        for mut frame in buffer.iter_samples() {
            let controls = Controls {
                timbre_semitones: p.timbre.value(),
                spread: p.spread.value(),
                scatter_ms: p.scatter.value(),
                density: p.density.value(),
                grain_size_ms: p.grain_size.value(),
                tone: p.tone.smoothed.next(),
                drive: p.drive.smoothed.next(),
                mix: p.mix.smoothed.next(),
                output_gain: p.output.smoothed.next(),
                freeze: p.freeze.value(),
                bypass: p.bypass.value(),
            };

            // Mono runs through the stereo engine with both sides the same
            let stereo = frame.len() > 1;
            let l = *frame.get_mut(0).unwrap();
            let r = if stereo {
                *frame.get_mut(1).unwrap()
            } else {
                l
            };
            let out = self.engine.process([l, r], &controls);

            if stereo {
                *frame.get_mut(0).unwrap() = out[0];
                *frame.get_mut(1).unwrap() = out[1];
            } else {
                *frame.get_mut(0).unwrap() = (out[0] + out[1]) * 0.5;
            }
        }

        ProcessStatus::Normal
    }
}

impl ClapPlugin for Grainbrew {
    const CLAP_ID: &'static str = "com.accraze.grainbrew";
    const CLAP_DESCRIPTION: Option<&'static str> =
        Some("Zero-latency time-domain granular timbre shifter");
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::AudioEffect,
        ClapFeature::Stereo,
        ClapFeature::Mono,
        ClapFeature::Granular,
    ];
}

impl Vst3Plugin for Grainbrew {
    const VST3_CLASS_ID: [u8; 16] = *b"GrainbrewTimbre!";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::PitchShift];
}

nih_export_clap!(Grainbrew);
nih_export_vst3!(Grainbrew);

#[cfg(test)]
mod tests {
    use super::*;

    /// Hosts type values back in as text, and CLAP validators check that this is stable.
    #[test]
    fn param_text_round_trips() {
        let p = GrainbrewParams::default();
        for param in [
            &p.timbre,
            &p.spread,
            &p.scatter,
            &p.density,
            &p.grain_size,
            &p.tone,
            &p.drive,
            &p.mix,
            &p.output,
        ] {
            for i in 0..=100 {
                let text = param.normalized_value_to_string(i as f32 / 100.0, true);
                let back = param
                    .string_to_normalized_value(&text)
                    .unwrap_or_else(|| panic!("{}: can't parse {text}", param.name()));
                assert_eq!(text, param.normalized_value_to_string(back, true));
            }
        }
    }
}
