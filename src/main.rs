use rust_graph::app::GraphApp;
use std::path::PathBuf;
fn main() -> eframe::Result {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--version") {
        println!("Rust Graph {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if args.iter().any(|a| a == "--help") {
        println!(
            "Rust Graph — native offline graphing calculator\nUsage: rust-graph [FILE.rustgraph]\n       rust-graph --version"
        );
        return Ok(());
    }
    let file = args.first().map(PathBuf::from);
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 850.0])
            .with_min_inner_size([800.0, 550.0])
            .with_app_id("rust-graph")
            .with_icon(
                eframe::icon_data::from_png_bytes(include_bytes!("../assets/rust-graph.png"))
                    .unwrap_or_default(),
            ),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        "Rust Graph",
        options,
        Box::new(move |cc| Ok(Box::new(GraphApp::new(cc, file.as_deref())))),
    )
}
