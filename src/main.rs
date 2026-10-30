//! Standalone app: runs the plugin directly against your audio and MIDI devices.
//! Run with `--help` to see the backend, device and MIDI options.

use nih_plug::prelude::*;

fn main() {
    nih_export_standalone::<proper_synth::Proper>();
}
