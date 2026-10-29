//! Luminance-based plot grid / axis ink for egui_plot.
//!
//! egui_plot paints grids and tick labels from `visuals.text_color()` (via
//! `color_from_strength`). Light chart backgrounds need dark ink; dark ones keep
//! quiet light ink so grids stay readable without washing out the series.

use egui::Color32;

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

/// Dark ink for light plot fills (grids + axis ticks/labels).
pub fn dark_plot_ink() -> Color32 {
    Color32::from_rgb(36, 36, 40)
}

/// Quiet light ink for dark plot fills (matches prior half-strength grids).
pub fn light_plot_ink() -> Color32 {
    Color32::from_rgb(230, 224, 240).gamma_multiply(0.40)
}

/// Grid / tick / label ink chosen from the chart Options background RGB.
pub fn plot_ink_for_background(bg: [u8; 3]) -> Color32 {
    if relative_luminance(bg) >= LIGHT_BG_THRESHOLD {
        dark_plot_ink()
    } else {
        light_plot_ink()
    }
}

/// Scope: plot fill from `bg`, contrast ink so grids + scales stay readable.
pub fn with_plot_contrast(ui: &mut egui::Ui, bg: [u8; 3], add: impl FnOnce(&mut egui::Ui)) {
    ui.scope(|ui| {
        ui.visuals_mut().extreme_bg_color = Color32::from_rgb(bg[0], bg[1], bg[2]);
        ui.visuals_mut().override_text_color = Some(plot_ink_for_background(bg));
        add(ui);
    });
}
