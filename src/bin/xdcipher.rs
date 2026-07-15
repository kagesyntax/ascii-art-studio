use xdcipher::app::AsciiApp;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 860.0])
            .with_title("xdcipher"),
        ..Default::default()
    };

    eframe::run_native(
        "xdcipher",
        options,
        Box::new(|_cc| Ok(Box::new(AsciiApp::default()))),
    )
}
