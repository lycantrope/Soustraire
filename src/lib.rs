#![warn(clippy::all, rust_2018_idioms)]

mod app;
pub use app::Subtractor;

pub fn load_icon() -> Option<eframe::IconData> {
    let icon = include_bytes!("../assets/icon-1024.png");
    let image = image::load_from_memory(icon).ok()?.into_rgba8();
    let (width, height) = image.dimensions();

    Some(eframe::IconData {
        rgba: image.into_raw(),
        width,
        height,
    })
}
