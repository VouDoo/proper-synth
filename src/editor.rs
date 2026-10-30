//! The GUI: the logo, a preset menu and one slider per parameter.
//!
//! The look follows the logo: cream on near-black like a club flyer, with
//! letter-spaced section titles echoing its tagline. Nothing else on purpose.
//!
//! egui redraws the whole window every frame by calling the closure in
//! `create`, so the code is just "draw this, then this". Each `ParamSlider`
//! reads and writes one parameter directly, and shows its value using the
//! formatting set up in `params.rs`.

use crate::params::{OpParams, SynthParams, set};
use crate::presets::PRESETS;
use nih_plug::prelude::*;
use nih_plug_egui::egui::{self, Color32, RichText, Ui};
use nih_plug_egui::resizable_window::ResizableWindow;
use nih_plug_egui::{create_egui_editor, widgets::ParamSlider};
use std::sync::Arc;

/// The logo's two colors, plus two greys in between.
const BLACK: Color32 = Color32::from_rgb(0x05, 0x05, 0x05);
const CREAM: Color32 = Color32::from_rgb(0xF4, 0xF0, 0xE8);
/// Section titles and the tagline.
const GREY: Color32 = Color32::from_rgb(0x8C, 0x88, 0x82);
/// Slider tracks and other backgrounds; a lighter one under the mouse.
const DARK: Color32 = Color32::from_rgb(0x1C, 0x1C, 0x1C);
const DARK_HOVER: Color32 = Color32::from_rgb(0x2E, 0x2E, 0x2E);

/// Width of a slider's bar. Its value is printed to the right of it, so each
/// slider takes up about this plus 70 pixels.
const SLIDER_WIDTH: f32 = 90.0;

pub fn create(params: Arc<SynthParams>) -> Option<Box<dyn Editor>> {
    let logo = logo();
    create_egui_editor(
        params.editor_state.clone(),
        (),
        // Runs once when the window opens.
        |ctx, _| {
            egui_extras::install_image_loaders(ctx);
            ctx.set_visuals(club_visuals());
        },
        // Runs every frame.
        move |ctx, setter, _| {
            // Drag the bottom-right corner to resize; the size is saved with
            // the plugin. If the window is ever smaller than its contents,
            // scrollbars appear instead of anything getting cut off.
            ResizableWindow::new("proper")
                .min_size([400.0, 200.0])
                .show(ctx, params.editor_state.as_ref(), |ui| {
                    egui::ScrollArea::both().show(ui, |ui| {
                        header(ui, &params, setter, &logo);
                        title(ui, "Sound");
                        sound_section(ui, &params, setter);
                        title(ui, "LFO");
                        lfo_section(ui, &params, setter);
                        title(ui, "Filter");
                        filter_section(ui, &params, setter);
                        title(ui, "Vowel");
                        vowel_section(ui, &params, setter);
                        title(ui, "Operators");
                        operator_section(ui, &params, setter);
                    });
                });
        },
    )
}

/// The logo, cropped to the globe and wordmark. Its tagline is hidden here
/// and shown as real text next to it instead: shrunk down to GUI size it
/// would be too small to read. (If the SVG changes and these edits no longer
/// match, the full logo is simply shown as is.)
fn logo() -> egui::ImageSource<'static> {
    let svg = include_str!("../media/logo.svg")
        .replacen(
            r#"width="1536" height="1024" viewBox="0 0 1536 1024""#,
            r#"width="944" height="824" viewBox="296 100 944 824""#,
            1,
        )
        .replacen(
            r#"inkscape:label="Tagline""#,
            r#"inkscape:label="Tagline" display="none""#,
            1,
        );
    egui::ImageSource::Bytes {
        uri: "bytes://logo.svg".into(),
        bytes: svg.into_bytes().into(),
    }
}

/// Cream on black, with dark grey tracks and cream slider fills.
fn club_visuals() -> egui::Visuals {
    let mut v = egui::Visuals::dark();
    v.panel_fill = BLACK;
    v.window_fill = BLACK;
    v.extreme_bg_color = DARK;
    // Filled part of sliders, and selected items (dark text on cream).
    v.selection.bg_fill = CREAM;
    v.selection.stroke.color = BLACK;
    // Text and lines.
    v.widgets.noninteractive.fg_stroke.color = CREAM;
    v.widgets.noninteractive.bg_stroke.color = DARK_HOVER;
    for widget in [
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
    ] {
        widget.fg_stroke.color = CREAM;
    }
    // Widget backgrounds.
    v.widgets.inactive.bg_fill = DARK;
    v.widgets.inactive.weak_bg_fill = DARK;
    v.widgets.hovered.bg_fill = DARK_HOVER;
    v.widgets.hovered.weak_bg_fill = DARK_HOVER;
    v.widgets.active.bg_fill = DARK_HOVER;
    v.widgets.active.weak_bg_fill = DARK_HOVER;
    v
}

/// "Filter" -> "F I L T E R": wide letter spacing, like the logo's tagline.
fn spaced(text: &str) -> String {
    let words: Vec<String> = text
        .to_uppercase()
        .split(' ')
        .map(|word| word.chars().map(String::from).collect::<Vec<_>>().join(" "))
        .collect();
    words.join("   ")
}

/// A section title, with a thin line above it.
fn title(ui: &mut Ui, text: &str) {
    ui.add_space(6.0);
    ui.separator();
    ui.label(RichText::new(spaced(text)).color(GREY).strong());
    ui.add_space(2.0);
}

/// The logo, with the tagline, preset menu and output levels beside it.
fn header(ui: &mut Ui, params: &SynthParams, setter: &ParamSetter, logo: &egui::ImageSource) {
    ui.horizontal(|ui| {
        ui.add(egui::Image::new(logo.clone()).fit_to_exact_size(egui::vec2(150.0, 131.0)));
        ui.add_space(20.0);
        ui.vertical(|ui| {
            ui.add_space(18.0);
            ui.label(RichText::new(spaced("A proper synth for house music")).color(GREY));
            ui.add_space(14.0);
            preset_menu(ui, params, setter);
            ui.add_space(8.0);
            egui::Grid::new("output").num_columns(4).show(ui, |ui| {
                ui.label("Gain");
                slider(ui, &params.gain, setter);
                ui.label("Preset Volume");
                slider(ui, &params.volume, setter);
                ui.end_row();
            });
        });
    });
}

fn preset_menu(ui: &mut Ui, params: &SynthParams, setter: &ParamSetter) {
    let mut current = params.preset_name.lock().unwrap();
    ui.horizontal(|ui| {
        ui.label("Preset");
        egui::ComboBox::from_id_salt("preset")
            .selected_text(current.clone())
            .width(180.0)
            .show_ui(ui, |ui| {
                for (name, patch) in PRESETS {
                    if ui.selectable_label(*current == *name, *name).clicked() {
                        params.load_patch(setter, patch);
                        *current = name.to_string();
                    }
                }
            });
    });
}

fn sound_section(ui: &mut Ui, params: &SynthParams, setter: &ParamSetter) {
    egui::Grid::new("sound").num_columns(6).show(ui, |ui| {
        ui.label("Algorithm");
        slider(ui, &params.algorithm, setter);
        ui.label("Feedback");
        slider(ui, &params.feedback, setter);
        ui.label("Key Track");
        slider(ui, &params.key_track, setter);
        ui.end_row();

        ui.label("Chorus");
        slider(ui, &params.chorus, setter);
        ui.label("Mono");
        checkbox(ui, &params.mono, setter);
        ui.label("Glide");
        slider(ui, &params.glide, setter);
        ui.end_row();
    });
}

fn lfo_section(ui: &mut Ui, params: &SynthParams, setter: &ParamSetter) {
    egui::Grid::new("lfo").num_columns(6).show(ui, |ui| {
        // Division is used when Sync is on, Rate when it's off.
        ui.label("Sync");
        checkbox(ui, &params.lfo_sync, setter);
        ui.label("Division");
        slider(ui, &params.lfo_division, setter);
        ui.label("Rate");
        slider(ui, &params.lfo_rate, setter);
        ui.end_row();

        ui.label("Pitch");
        slider(ui, &params.lfo_pitch, setter);
        ui.label("Wobble");
        slider(ui, &params.lfo_wobble, setter);
        ui.label("Volume");
        slider(ui, &params.lfo_volume, setter);
        ui.end_row();

        ui.label("Cutoff");
        slider(ui, &params.lfo_cutoff, setter);
        ui.end_row();
    });
}

fn filter_section(ui: &mut Ui, params: &SynthParams, setter: &ParamSetter) {
    let f = &params.filter;
    egui::Grid::new("filter").num_columns(6).show(ui, |ui| {
        ui.label("Cutoff");
        slider(ui, &f.cutoff, setter);
        ui.label("Resonance");
        slider(ui, &f.resonance, setter);
        ui.label("Env Amount");
        slider(ui, &f.env_amount, setter);
        ui.end_row();

        ui.label("Attack");
        slider(ui, &f.attack, setter);
        ui.label("Decay");
        slider(ui, &f.decay, setter);
        ui.label("Sustain");
        slider(ui, &f.sustain, setter);
        ui.end_row();

        ui.label("Release");
        slider(ui, &f.release, setter);
        ui.label("Key Track");
        slider(ui, &f.key_track, setter);
        ui.label("Velocity");
        slider(ui, &f.velocity, setter);
        ui.end_row();
    });
}

fn vowel_section(ui: &mut Ui, params: &SynthParams, setter: &ParamSetter) {
    egui::Grid::new("vowel").num_columns(4).show(ui, |ui| {
        ui.label("Vowel");
        slider(ui, &params.vowel, setter);
        ui.label("Mix");
        slider(ui, &params.vowel_mix, setter);
        ui.end_row();
    });
}

/// A table: one row per setting, one column per operator.
fn operator_section(ui: &mut Ui, params: &SynthParams, setter: &ParamSetter) {
    egui::Grid::new("operators").num_columns(5).show(ui, |ui| {
        ui.label("");
        for number in 1..=params.ops.len() {
            ui.label(RichText::new(number.to_string()).color(GREY).strong());
        }
        ui.end_row();

        operator_row(ui, "Ratio", params, setter, |op| &op.ratio);
        operator_row(ui, "Level", params, setter, |op| &op.level);
        operator_row(ui, "Velocity", params, setter, |op| &op.velocity);
        operator_row(ui, "Attack", params, setter, |op| &op.attack);
        operator_row(ui, "Decay", params, setter, |op| &op.decay);
        operator_row(ui, "Sustain", params, setter, |op| &op.sustain);
        operator_row(ui, "Release", params, setter, |op| &op.release);
    });
}

/// One row of the operator table: the same setting for all 4 operators.
/// `param_of` picks which setting, e.g. `|op| &op.ratio`.
fn operator_row(
    ui: &mut Ui,
    label: &str,
    params: &SynthParams,
    setter: &ParamSetter,
    param_of: impl Fn(&OpParams) -> &FloatParam,
) {
    ui.label(label);
    for op in &params.ops {
        slider(ui, param_of(op), setter);
    }
    ui.end_row();
}

/// An on/off switch for a `BoolParam`.
fn checkbox(ui: &mut Ui, param: &BoolParam, setter: &ParamSetter) {
    let mut on = param.value();
    if ui.checkbox(&mut on, "").changed() {
        set(setter, param, on);
    }
}

fn slider<P: Param>(ui: &mut Ui, param: &P, setter: &ParamSetter) {
    ui.add(ParamSlider::for_param(param, setter).with_width(SLIDER_WIDTH));
}
