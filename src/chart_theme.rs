//! Luminance-based plot grid / axis ink for egui_plot.
//!
//! egui_plot paints the data-area fill from `extreme_bg_color`, grids and tick
//! labels from `text_color()` (via `color_from_strength`), and axis titles from
//! `override_text_color`. Axis margins are not filled by egui_plot — we paint
//! the full widget rect so chrome matches the Options chart background.
//!
//! Light chart backgrounds need quiet dark ink; dark ones keep quiet light ink
//! so grids stay readable without washing out the series.

use egui::{Color32, Rounding};

/// WCAG relative luminance of an sRGB triplet (0..=1).
pub fn relative_luminance(rgb: [u8; 3]) -> f32 {
    fn channel(c: u8) -> f32 {
        let c = c as f32 / 255.0;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    }
    0.2126 * channel(rgb[0]) + 0.7152 * channel(rgb[1]) + 0.0722 * channel(rgb[2])
}

/// Backgrounds at or above this luminance get dark grid/scale ink.
pub const LIGHT_BG_THRESHOLD: f32 = 0.5;

/// Shared fade so grids stay ~half-strength (matches prior quiet light grids).
const INK_FADE: f32 = 0.40;

/// Dark ink for light plot fills (grids + axis ticks/labels) — faded, not solid.
pub fn dark_plot_ink() -> Color32 {
    Color32::from_rgb(36, 36, 40).gamma_multiply(INK_FADE)
}

/// Quiet light ink for dark plot fills (matches prior half-strength grids).
pub fn light_plot_ink() -> Color32 {
    Color32::from_rgb(230, 224, 240).gamma_multiply(INK_FADE)
}

/// Grid / tick / label ink chosen from the chart Options background RGB.
pub fn plot_ink_for_background(bg: [u8; 3]) -> Color32 {
    if relative_luminance(bg) >= LIGHT_BG_THRESHOLD {
        dark_plot_ink()
    } else {
        light_plot_ink()
    }
}

/// Scope: fill axes + plot chrome from `bg`, contrast ink for grids + scales.
pub fn with_plot_contrast(ui: &mut egui::Ui, bg: [u8; 3], add: impl FnOnce(&mut egui::Ui)) {
    ui.scope(|ui| {
        let fill = Color32::from_rgb(bg[0], bg[1], bg[2]);
        let ink = plot_ink_for_background(bg);
        {
            let v = ui.visuals_mut();
            // Data-area fill (egui_plot RectShape) + any panel/widget chrome behind axes.
            v.extreme_bg_color = fill;
            v.panel_fill = fill;
            v.faint_bg_color = fill;
            v.widgets.noninteractive.bg_fill = fill;
            // Plot frame stroke follows ink so borders don't clash on light fills.
            v.widgets.noninteractive.bg_stroke =
                egui::Stroke::new(1.0_f32, ink.gamma_multiply(0.55));
            // Grids (color_from_strength) + axis titles + tick labels.
            v.override_text_color = Some(ink);
        }
        // egui_plot only paints plot_rect — fill the full widget (X/Y margins) first.
        let rect = ui.available_rect_before_wrap();
        ui.painter().rect_filled(rect, Rounding::same(2.0), fill);
        add(ui);
    });
}
