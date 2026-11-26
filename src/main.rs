//! Web-Abnahme: describe use cases in your own words, let Claude Code derive an
//! acceptance checklist and test the web app in Chrome or Firefox.

mod app;
mod claude;
mod fixbrief;
mod mcp_server;
mod model;
mod reextract;
mod report;
mod shell;
mod store;
mod tab_blocked;
mod tab_checklist;
mod tab_fix;
mod tab_run;
mod tab_signoff;
mod tab_spec;
mod theme;

use eframe::egui;
use std::path::{Path, PathBuf};

pub fn save_png(path: &Path, img: &egui::ColorImage) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let w = std::io::BufWriter::new(file);
    let mut enc = png::Encoder::new(w, img.size[0] as u32, img.size[1] as u32);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut writer = enc.write_header().map_err(|e| e.to_string())?;
    let mut bytes = Vec::with_capacity(img.pixels.len() * 4);
    for p in &img.pixels {
        bytes.extend_from_slice(&[p.r(), p.g(), p.b(), 255]);
    }
    writer.write_image_data(&bytes).map_err(|e| e.to_string())
}

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--mcp-ergebnis") {
        mcp_server::run_stdio();
        return Ok(());
    }
    let screenshot = args
        .iter()
        .position(|a| a == "--screenshot")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from);
    let start_tab = args
        .iter()
        .position(|a| a == "--tab")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.parse::<u8>().ok());

    let mut options = eframe::NativeOptions {
        initial_window_size: Some(egui::vec2(1320.0, 840.0)),
        min_window_size: Some(egui::vec2(980.0, 640.0)),
        centered: true,
        app_id: Some("web-abnahme".into()),
        ..Default::default()
    };
    if screenshot.is_some() {
        options.active = false;
        #[cfg(target_os = "macos")]
        {
            options.event_loop_builder = Some(Box::new(|b| {
                use winit::platform::macos::EventLoopBuilderExtMacOS;
                b.with_activate_ignoring_other_apps(false);
            }));
        }
        #[cfg(target_os = "windows")]
        {
            options.always_on_top = false;
        }
    }
    eframe::run_native(
        "Web-Abnahme",
        options,
        Box::new(move |cc| Box::new(app::App::new(cc, screenshot, start_tab))),
    )
}
