#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod app_dir;
mod app_state;
mod bookmarks;
mod color_profiles;
mod display_color;
mod input;
mod io_worker;
mod j_preview;
mod navigation;
mod preferences;
mod render_bridge;
mod ui;

fn main() -> eframe::Result {
    app::run()
}
