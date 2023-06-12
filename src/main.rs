#![warn(clippy::all, rust_2018_idioms)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

// When compiling natively:
#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result<()> {
    // Log to stdout (if you run with `RUST_LOG=debug`).
    tracing_subscriber::fmt::init();

    let native_options = eframe::NativeOptions {
        icon_data: soustraire::load_icon(),
        transparent: true,
        centered: true,
        follow_system_theme: false,
        ..Default::default()
    };
    eframe::run_native(
        "Soustraire",
        native_options,
        Box::new(|cc| Box::new(soustraire::Subtractor::new(cc))),
    )
}

// when compiling to web using trunk.
#[cfg(target_arch = "wasm32")]
fn main() {
    rayon::ThreadPoolBuilder::new()
        .num_threads(4)
        .build_global()?;
    // Make sure panics are logged using `console.error`.
    console_error_panic_hook::set_once();

    // Redirect tracing to console.log and friends:
    tracing_wasm::set_as_global_default();

    let web_options = eframe::WebOptions::default();

    wasm_bindgen_futures::spawn_local(async {
        eframe::start_web(
            "the_canvas_id", // hardcode it
            web_options,
            Box::new(|cc| Box::new(soustraire::Subtractor::new(cc))),
        )
        .await
        .expect("failed to start eframe");
    });
}
