//! One knob per parameter. `knob()` is the same widget as the other plugins here.
//! Drag or scroll to turn, Shift for fine, double-click to reset.

use crate::GrainbrewParams;
use nih_plug::prelude::*;
use nih_plug_egui::create_egui_editor;
use nih_plug_egui::egui::{
    self, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2,
};
use std::f32::consts::{PI, TAU};
use std::sync::Arc;

pub const WIDTH: u32 = 550;
pub const HEIGHT: u32 = 270;

const KNOB_RADIUS: f32 = 26.0;
/// Knobs turn +-135 degrees from straight up
const SWEEP: f32 = 0.75 * PI;
const WHITE: Color32 = Color32::from_rgb(236, 234, 228);

pub fn create(params: Arc<GrainbrewParams>) -> Option<Box<dyn Editor>> {
    create_egui_editor(
        params.editor_state.clone(),
        (),
        |_, _| {},
        move |ctx, setter, _| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let knobs = [
                    &params.timbre,
                    &params.spread,
                    &params.scatter,
                    &params.density,
                    &params.grain_size,
                    &params.tone,
                    &params.drive,
                    &params.mix,
                    &params.output,
                ];
                // The real width, because a saved project restores the window size it was saved at
                let width = ctx.screen_rect().width();
                let cell = |i: usize| {
                    Pos2::new(
                        width * ((i % 5) as f32 + 0.5) / 5.0,
                        75.0 + 125.0 * (i / 5) as f32,
                    )
                };
                for (i, param) in knobs.iter().enumerate() {
                    knob(ui, setter, param, cell(i));
                }
                let toggles = cell(knobs.len());
                toggle(ui, setter, &params.freeze, toggles - Vec2::new(0.0, 23.0));
                toggle(ui, setter, &params.bypass, toggles + Vec2::new(0.0, 23.0));
            });
        },
    )
}

fn toggle(ui: &Ui, setter: &ParamSetter, param: &BoolParam, center: Pos2) {
    let rect = Rect::from_center_size(center, Vec2::new(72.0, 36.0));
    let response = ui.interact(rect, ui.id().with(param.name()), Sense::click());
    if response.clicked() {
        setter.begin_set_parameter(param);
        setter.set_parameter(param, !param.value());
        setter.end_set_parameter(param);
    }

    let on = param.value();
    let p = ui.painter();
    p.rect_filled(
        rect,
        6.0,
        if on {
            Color32::from_rgb(120, 190, 235)
        } else {
            Color32::from_gray(26)
        },
    );
    p.rect_stroke(
        rect,
        6.0,
        Stroke::new(1.0_f32, Color32::from_gray(90)),
        StrokeKind::Outside,
    );
    p.text(
        center,
        Align2::CENTER_CENTER,
        param.name().to_uppercase(),
        FontId::proportional(13.0),
        if on { Color32::from_gray(10) } else { WHITE },
    );
}

/// Unit vector for a knob angle, measured clockwise from straight up.
fn dir(angle: f32) -> Vec2 {
    Vec2::new(angle.sin(), -angle.cos())
}

fn knob(ui: &Ui, setter: &ParamSetter, param: &FloatParam, center: Pos2) {
    let rect = Rect::from_center_size(center, Vec2::splat(2.0 * KNOB_RADIUS + 12.0));
    let response = ui.interact(rect, ui.id().with(param.name()), Sense::click_and_drag());

    // Accumulate drags in egui memory since the param value can lag behind the host round trip
    if response.drag_started() {
        setter.begin_set_parameter(param);
        ui.data_mut(|d| d.insert_temp(response.id, param.unmodulated_normalized_value()));
    }
    if response.dragged() {
        let speed = if ui.input(|i| i.modifiers.shift) {
            0.0005
        } else {
            0.005
        };
        let delta = response.drag_delta();
        let start = ui
            .data(|d| d.get_temp(response.id))
            .unwrap_or(param.unmodulated_normalized_value());
        let value = (start + (delta.x - delta.y) * speed).clamp(0.0, 1.0);
        ui.data_mut(|d| d.insert_temp(response.id, value));
        setter.set_parameter_normalized(param, value);
    }
    if response.drag_stopped() {
        setter.end_set_parameter(param);
    }
    if response.double_clicked() {
        setter.begin_set_parameter(param);
        setter.set_parameter(param, param.default_plain_value());
        setter.end_set_parameter(param);
    }

    if response.hovered() {
        let (notches, shift) = ui.input(|i| {
            let notches: f32 = i
                .events
                .iter()
                .map(|e| match e {
                    // Smooth-scrolling touchpads report points; treat ~50 points as one notch
                    egui::Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        delta,
                        ..
                    } => delta.y / 50.0,
                    egui::Event::MouseWheel { delta, .. } => delta.y,
                    _ => 0.0,
                })
                .sum();
            (notches, i.modifiers.shift)
        });
        if notches != 0.0 {
            let step = if shift { 0.002 } else { 0.02 };
            let value = (param.unmodulated_normalized_value() + notches * step).clamp(0.0, 1.0);
            setter.begin_set_parameter(param);
            setter.set_parameter_normalized(param, value);
            setter.end_set_parameter(param);
        }
    }

    let value = param.unmodulated_normalized_value();
    let angle = -SWEEP + 2.0 * SWEEP * value;
    let p = ui.painter();

    for i in 0..=10 {
        let a = -SWEEP + 2.0 * SWEEP * i as f32 / 10.0;
        let r = if i % 5 == 0 { 2.0 } else { 1.2 };
        p.circle_filled(center + dir(a) * (KNOB_RADIUS + 9.0), r, WHITE);
    }

    // Knurled skirt that turns with the knob
    p.circle_filled(
        center + Vec2::new(0.0, 3.0),
        KNOB_RADIUS,
        Color32::from_black_alpha(160),
    );
    p.circle_filled(center, KNOB_RADIUS, Color32::from_gray(8));
    for i in 0..40 {
        let a = angle + i as f32 * TAU / 40.0;
        p.line_segment(
            [
                center + dir(a) * (KNOB_RADIUS - 6.0),
                center + dir(a) * (KNOB_RADIUS - 0.5),
            ],
            Stroke::new(1.5_f32, Color32::from_gray(42)),
        );
    }
    p.circle_filled(center, KNOB_RADIUS - 7.0, Color32::from_gray(26));
    p.circle_stroke(
        center,
        KNOB_RADIUS - 7.0,
        Stroke::new(1.0_f32, Color32::from_gray(58)),
    );
    p.line_segment(
        [
            center + dir(angle) * 7.0,
            center + dir(angle) * (KNOB_RADIUS - 9.0),
        ],
        Stroke::new(3.0_f32, WHITE),
    );

    p.text(
        center - Vec2::new(0.0, KNOB_RADIUS + 18.0),
        Align2::CENTER_BOTTOM,
        param.name().to_uppercase(),
        FontId::proportional(13.0),
        WHITE,
    );
    if response.hovered() || response.dragged() {
        p.text(
            center + Vec2::new(0.0, KNOB_RADIUS + 16.0),
            Align2::CENTER_TOP,
            param.normalized_value_to_string(value, true),
            FontId::proportional(12.0),
            Color32::from_gray(170),
        );
    }
}
