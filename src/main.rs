#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use chart_bench::app::{load_window_icon, ChartBenchApp};
use eframe::egui;

fn main() -> eframe::Result<()> {
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([1280.0, 800.0])
        .with_min_inner_size([800.0, 500.0])
        .with_title(format!("Chart-Bench  {}", env!("CARGO_PKG_VERSION")))
        // Required for OS file DnD (esp. Windows); harmless elsewhere.
        .with_drag_and_drop(true);
    if let Some(icon) = load_window_icon() {
        viewport = viewport.with_icon(icon);
    }

    let mut options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    // winit's Wayland backend does not emit DroppedFile / HoveredFile. Prefer X11
    // (XWayland under Hyprland/Omarchy) whenever DISPLAY is set so file-manager
    // drops into Files actually load.
    #[cfg(target_os = "linux")]
    {
        if std::env::var_os("DISPLAY").is_some() {
            options.event_loop_builder = Some(Box::new(|builder| {
                use winit::platform::x11::EventLoopBuilderExtX11;
                builder.with_x11();
            }));
        }
    }
    #[cfg(not(target_os = "linux"))]
    let _ = &mut options;

    eframe::run_native(
        "chart-bench",
        options,
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Ok(Box::<ChartBenchApp>::default())
        }),
    )
}
