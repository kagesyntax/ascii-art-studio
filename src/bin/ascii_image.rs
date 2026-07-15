use image::GenericImageView;
use std::env;
use std::path::Path;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <image_path> [scale]", args[0]);
        std::process::exit(1);
    }

    let path = &args[1];
    let scale: u32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(4);

    let img = image::open(Path::new(path)).unwrap_or_else(|e| {
        eprintln!("Failed to open image: {}", e);
        std::process::exit(1);
    });

    let (w, h) = img.dimensions();
    let chars = ["@", "#", "S", "%", "?", "*", "+", ";", ":", ",", "."];

    for y in (0..h).step_by(scale as usize) {
        for x in (0..w).step_by(scale as usize) {
            let px = img.get_pixel(x, y);
            let gray = 0.299 * px[0] as f32 + 0.587 * px[1] as f32 + 0.114 * px[2] as f32;
            let idx = (gray / 255.0 * (chars.len() - 1) as f32).round() as usize;
            print!("{}", chars[idx]);
        }
        println!();
    }
}
