//! Crop a viewport screenshot to a plot rect and write PNG via save dialog.

use std::path::{Path, PathBuf};

use egui::ColorImage;

/// Open a save dialog defaulting to `{title}.png`.
pub fn save_png_dialog(title: &str) -> Option<PathBuf> {
    let name = format!("{}.png", sanitize_filename(title));
    rfd::FileDialog::new()
        .add_filter("PNG image", &["png"])
        .set_file_name(name)
        .save_file()
}

/// Crop `full` to `plot_rect` (egui points) and write PNG to `path`.
pub fn write_cropped_png(
    path: &Path,
    full: &ColorImage,
    plot_rect: egui::Rect,
    pixels_per_point: f32,
) -> anyhow::Result<()> {
    let cropped = crop_to_plot(full, plot_rect, pixels_per_point)?;
    write_color_image_png(path, &cropped)
}

/// Full flow: dialog → crop → encode. Returns the path written.
pub fn save_plot_png(
    title: &str,
    full: &ColorImage,
    plot_rect: egui::Rect,
    pixels_per_point: f32,
) -> anyhow::Result<PathBuf> {
    let path = save_png_dialog(title).ok_or_else(|| anyhow::anyhow!("cancelled"))?;
    write_cropped_png(&path, full, plot_rect, pixels_per_point)?;
    Ok(path)
}

/// Crop screenshot pixels to the plot rect (points → pixels, clamped).
fn crop_to_plot(
    full: &ColorImage,
    plot_rect: egui::Rect,
    pixels_per_point: f32,
) -> anyhow::Result<ColorImage> {
    if !plot_rect.is_finite() || plot_rect.width() < 1.0 || plot_rect.height() < 1.0 {
        anyhow::bail!("plot rect unavailable — try again after the chart paints");
    }
    let ppp = pixels_per_point.max(0.01);
    let w = full.size[0];
    let h = full.size[1];
    if w == 0 || h == 0 {
        anyhow::bail!("empty screenshot");
    }
    // Convert points → pixel indices, then clamp inside the image.
    let mut min_x = (plot_rect.min.x * ppp).floor().max(0.0) as usize;
    let mut min_y = (plot_rect.min.y * ppp).floor().max(0.0) as usize;
    let mut max_x = (plot_rect.max.x * ppp).ceil() as usize;
    let mut max_y = (plot_rect.max.y * ppp).ceil() as usize;
    max_x = max_x.min(w);
    max_y = max_y.min(h);
    if max_x <= min_x {
        min_x = min_x.min(w.saturating_sub(1));
        max_x = (min_x + 1).min(w);
    }
    if max_y <= min_y {
        min_y = min_y.min(h.saturating_sub(1));
        max_y = (min_y + 1).min(h);
    }
    // region() scales by pixels_per_point — pass pixel coords with ppp=1.
    let px = egui::Rect::from_min_max(
        egui::pos2(min_x as f32, min_y as f32),
        egui::pos2(max_x as f32, max_y as f32),
    );
    Ok(full.region(&px, Some(1.0)))
}

/// Encode a ColorImage as PNG (straight RGBA).
fn write_color_image_png(path: &Path, image: &ColorImage) -> anyhow::Result<()> {
    let mut rgba = Vec::with_capacity(image.pixels.len() * 4);
    for p in &image.pixels {
        let [r, g, b, a] = p.to_srgba_unmultiplied();
        rgba.extend_from_slice(&[r, g, b, a]);
    }
    image::save_buffer(
        path,
        &rgba,
        image.size[0] as u32,
        image.size[1] as u32,
        image::ColorType::Rgba8,
    )?;
    Ok(())
}

fn sanitize_filename(title: &str) -> String {
    let s: String = title
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    let s = s.trim().replace(' ', "_");
    if s.is_empty() {
        "chart".into()
    } else {
        s
    }
}
