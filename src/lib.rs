//! Proper: a 4-operator FM synthesizer for house and UK garage.
//!
//! - `dsp/`       makes the sound (start with `dsp/mod.rs`)
//! - `presets.rs` the factory sounds
//! - `params.rs`  the knobs a DAW sees
//! - `editor.rs`  the GUI window
//! - this file    the nih-plug glue: MIDI in, audio out
//!
//! The same code builds as a CLAP/VST3 plugin and as the standalone app in
//! `src/main.rs`.

pub mod dsp;
mod editor;
pub mod params;
pub mod presets;

use dsp::Synth;
use nih_plug::prelude::*;
use params::SynthParams;
use std::sync::Arc;

/// How often the knobs are read, in samples (32 = every 0.7 ms at 48 kHz).
/// Often enough that parameter changes sound smooth, rarely enough that the
/// slow setup math in `Voice::prepare` costs almost nothing.
const CONTROL_BLOCK: usize = 32;

pub struct Proper {
    params: Arc<SynthParams>,
    synth: Synth,
}

impl Default for Proper {
    fn default() -> Self {
        let (name, patch) = presets::PRESETS[0];

        Self {
            params: Arc::new(SynthParams::new(name, &patch)),
            synth: Synth::new(48_000.0, &patch),
        }
    }
}

impl Plugin for Proper {
    const NAME: &'static str = "Proper";
    const VENDOR: &'static str = "voudoo";
    const URL: &'static str = "https://github.com/VouDoo/proper-synth";
    const EMAIL: &'static str = "";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout {
        main_input_channels: None,
        main_output_channels: NonZeroU32::new(2),
        ..AudioIOLayout::const_default()
    }];

    // Notes only: knobs on a controller are mapped to parameters in the DAW.
    const MIDI_INPUT: MidiConfig = MidiConfig::Basic;
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
        self.synth.set_sample_rate(buffer_config.sample_rate);
        // Tell the DAW the output is slightly late (oversampling filter and
        // limiter lookahead), so it can shift it back in line with the
        // other tracks.
        context.set_latency_samples(self.synth.latency());
        true
    }

    fn reset(&mut self) {
        // Start at the Gain knob's level, rather than gliding to it from 0 dB.
        self.synth.set_gain(self.params.gain.value());
        self.synth.reset();
    }

    // This runs on the real-time audio thread: no allocation, locks or I/O.
    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let mut next_event = context.next_event();

        for (block_start, mut block) in buffer.iter_blocks(CONTROL_BLOCK) {
            // Follow the DAW's tempo (the standalone app has `--tempo`).
            let tempo = context.transport().tempo.unwrap_or(120.0);
            self.synth.set_tempo(tempo as f32);
            let patch = self.params.to_patch(block.samples() as u32);
            self.synth.set_patch(&patch);
            self.synth.set_gain(self.params.gain.value());

            for (i, channel_samples) in block.iter_samples().enumerate() {
                // Apply every MIDI event scheduled at (or before) this sample,
                // so note timing is sample-accurate, not snapped to the block.
                let now = (block_start + i) as u32;
                while let Some(event) = next_event {
                    if event.timing() > now {
                        break;
                    }
                    match event {
                        NoteEvent::NoteOn { note, velocity, .. } => {
                            self.synth.note_on(note, velocity)
                        }
                        NoteEvent::NoteOff { note, .. } => self.synth.note_off(note),
                        _ => {}
                    }
                    next_event = context.next_event();
                }

                let (left, right) = self.synth.next_sample();
                let mut channels = channel_samples.into_iter();
                *channels.next().unwrap() = left;
                *channels.next().unwrap() = right;
            }
        }

        ProcessStatus::KeepAlive
    }
}

impl ClapPlugin for Proper {
    const CLAP_ID: &'static str = "com.voudoo.proper";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("A 4-operator FM synthesizer");
    const CLAP_MANUAL_URL: Option<&'static str> = Some(Self::URL);
    const CLAP_SUPPORT_URL: Option<&'static str> =
        Some("https://github.com/VouDoo/proper-synth/issues");
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::Instrument,
        ClapFeature::Synthesizer,
        ClapFeature::Stereo,
    ];
}

impl Vst3Plugin for Proper {
    const VST3_CLASS_ID: [u8; 16] = *b"voudooProperFM01";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Instrument, Vst3SubCategory::Synth];
}

nih_export_clap!(Proper);
// The VST3 bindings inside this macro use an old macro style that Rust 1.99+
// warns about. It's harmless and not our code, so silence it for this macro
// only. (A lint `allow` can't go on a macro call directly, hence the module;
// `unknown_lints` keeps older Rust versions, which lack this lint, quiet.)
#[allow(unknown_lints, semicolon_in_expressions_from_non_local_macros)]
mod vst3_export {
    use super::*;
    nih_export_vst3!(Proper);
}
