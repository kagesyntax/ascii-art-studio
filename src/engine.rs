use image::{DynamicImage, GenericImageView};

#[cfg(not(target_arch = "wasm32"))]
use rayon::prelude::*;

pub const CHARSETS: &[&str] = &[
    "@%#*+=-:. ",
    "@#S%?*+;:,.",
    "$@B%8&WM#*oahkbdpqwmZO0QLCJUYXzcvunxrjft/\\|()1{}[]?-_+~<>i!lI;:,\"^`. ",
    "█▓▒░ ",
];

pub const CHARSET_NAMES: &[&str] = &["10-step", "Compact", "Bourke 70", "Unicode Blocks"];

pub const EDGE_CHARS: &[char] = &['-', '/', '|', '\\'];

fn luminance(r: u8, g: u8, b: u8) -> f32 {
    0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32
}

fn sobel_edges(gray: &[f32], w: usize, h: usize, threshold: f32) -> Vec<Option<usize>> {
    let mut edges = vec![None; w * h];
    let t2 = threshold * threshold;

    #[cfg(not(target_arch = "wasm32"))]
    edges.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
        if y == 0 || y == h - 1 {
            return;
        }
        let row_base = (y - 1) * w;
        for x in 1..w - 1 {
            let base = row_base + (x - 1);
            let mut gx = 0.0;
            let mut gy = 0.0;
            for ky in 0..3 {
                let r = &gray[base + ky * w..];
                gx += r[0] * -1.0 + r[2] * 1.0;
                gy += r[0] * -1.0 + r[1] * -2.0 + r[2] * -1.0;
            }
            let mag2 = gx * gx + gy * gy;
            if mag2 > t2 {
                let gy_abs = gy.abs();
                let gx_abs = gx.abs();
                let dir = if gy_abs > 2.414 * gx_abs {
                    0
                } else if gx_abs > 2.414 * gy_abs {
                    2
                } else if (gx > 0.0) == (gy > 0.0) {
                    3
                } else {
                    1
                };
                row[x] = Some(dir);
            }
        }
    });

    #[cfg(target_arch = "wasm32")]
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let base = (y - 1) * w + (x - 1);
            let mut gx = 0.0;
            let mut gy = 0.0;
            for ky in 0..3 {
                let r = &gray[base + ky * w..];
                gx += r[0] * -1.0 + r[2] * 1.0;
                gy += r[0] * -1.0 + r[1] * -2.0 + r[2] * -1.0;
            }
            let mag2 = gx * gx + gy * gy;
            if mag2 > t2 {
                let gy_abs = gy.abs();
                let gx_abs = gx.abs();
                let dir = if gy_abs > 2.414 * gx_abs {
                    0
                } else if gx_abs > 2.414 * gy_abs {
                    2
                } else if (gx > 0.0) == (gy > 0.0) {
                    3
                } else {
                    1
                };
                edges[y * w + x] = Some(dir);
            }
        }
    }

    edges
}

#[derive(Clone)]
pub struct AsciiCell {
    pub ch: char,
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

#[derive(Clone)]
pub struct AsciiResult {
    pub cells: Vec<Vec<AsciiCell>>,
    pub img_w: u32,
    pub img_h: u32,
    pub out_w: usize,
    pub out_h: usize,
}

#[derive(Clone)]
pub struct AsciiConfig {
    pub width_chars: usize,
    pub color: bool,
    pub dither: bool,
    pub edges: bool,
    pub invert: bool,
    pub contrast: f32,
    pub edge_threshold: f32,
    pub charset_index: usize,
    pub font_size: f32,
}

impl Default for AsciiConfig {
    fn default() -> Self {
        Self {
            width_chars: 160,
            color: false,
            dither: false,
            edges: false,
            invert: false,
            contrast: 1.0,
            edge_threshold: 40.0,
            charset_index: 0,
            font_size: 8.0,
        }
    }
}

pub fn convert_to_ascii(img: &DynamicImage, config: &AsciiConfig) -> Option<AsciiResult> {
    let (img_w, img_h) = img.dimensions();
    if img_w == 0 || img_h == 0 {
        return None;
    }

    let out_w = config.width_chars;
    let aspect_ratio = 0.45;
    let out_h = ((img_h as f32 / img_w as f32) * out_w as f32 * aspect_ratio).round() as usize;
    let out_h = out_h.max(1);

    let resized = img.resize_exact(
        out_w as u32,
        out_h as u32,
        image::imageops::FilterType::Nearest,
    );

    let charset = CHARSETS[config.charset_index];
    let chars: Vec<char> = charset.chars().collect();
    let num_levels = chars.len();

    let n_pixels = out_w * out_h;

    #[cfg(not(target_arch = "wasm32"))]
    let mut pixels = {
        let mut px = vec![(0.0f32, 0u8, 0u8, 0u8); n_pixels];
        px.par_chunks_mut(out_w).enumerate().for_each(|(y, row)| {
            for (x, cell) in row.iter_mut().enumerate() {
                let c = resized.get_pixel(x as u32, y as u32);
                *cell = (luminance(c[0], c[1], c[2]), c[0], c[1], c[2]);
            }
        });
        px
    };

    #[cfg(target_arch = "wasm32")]
    let mut pixels = {
        let mut px = Vec::with_capacity(n_pixels);
        for y in 0..out_h {
            for x in 0..out_w {
                let c = resized.get_pixel(x as u32, y as u32);
                px.push((luminance(c[0], c[1], c[2]), c[0], c[1], c[2]));
            }
        }
        px
    };

    if config.contrast != 1.0 {
        #[cfg(not(target_arch = "wasm32"))]
        pixels.par_iter_mut().for_each(|px| {
            let l = px.0 / 255.0;
            px.0 = (l.powf(config.contrast) * 255.0).clamp(0.0, 255.0);
        });
        #[cfg(target_arch = "wasm32")]
        for px in &mut pixels {
            let l = px.0 / 255.0;
            px.0 = (l.powf(config.contrast) * 255.0).clamp(0.0, 255.0);
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    let mut luma_vals: Vec<f32> = pixels.par_iter().map(|p| p.0).collect();
    #[cfg(target_arch = "wasm32")]
    let mut luma_vals: Vec<f32> = pixels.iter().map(|p| p.0).collect();
    let edge_map = if config.edges {
        Some(sobel_edges(&luma_vals, out_w, out_h, config.edge_threshold))
    } else {
        None
    };

    if config.dither {
        for y in 0..out_h {
            for x in 0..out_w {
                let idx = y * out_w + x;
                let old = luma_vals[idx];
                let level = (old / 255.0 * (num_levels - 1) as f32).round() as usize;
                let level = level.min(num_levels - 1);
                let new = level as f32 / (num_levels - 1) as f32 * 255.0;
                let err = old - new;
                luma_vals[idx] = new;

                if x + 1 < out_w {
                    luma_vals[y * out_w + x + 1] += err * 7.0 / 16.0;
                }
                if y + 1 < out_h {
                    if x > 0 {
                        luma_vals[(y + 1) * out_w + x - 1] += err * 3.0 / 16.0;
                    }
                    luma_vals[(y + 1) * out_w + x] += err * 5.0 / 16.0;
                    if x + 1 < out_w {
                        luma_vals[(y + 1) * out_w + x + 1] += err * 1.0 / 16.0;
                    }
                }
            }
        }
    }

    let invert = config.invert;

    #[cfg(not(target_arch = "wasm32"))]
    let cells: Vec<Vec<AsciiCell>> = (0..out_h)
        .into_par_iter()
        .map(|y| {
            build_row(y, out_w, &pixels, &luma_vals, &edge_map, &chars, num_levels, invert)
        })
        .collect();

    #[cfg(target_arch = "wasm32")]
    let cells: Vec<Vec<AsciiCell>> = (0..out_h)
        .map(|y| {
            build_row(y, out_w, &pixels, &luma_vals, &edge_map, &chars, num_levels, invert)
        })
        .collect();

    Some(AsciiResult {
        cells,
        img_w,
        img_h,
        out_w,
        out_h,
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn convert_frame(img: &DynamicImage, config: &AsciiConfig) -> Option<AsciiResult> {
    let (img_w, img_h) = img.dimensions();
    if img_w == 0 || img_h == 0 {
        return None;
    }

    let out_w = config.width_chars;
    let aspect_ratio = 0.45;
    let out_h = ((img_h as f32 / img_w as f32) * out_w as f32 * aspect_ratio).round() as usize;
    let out_h = out_h.max(1);

    let resized = img.resize_exact(
        out_w as u32,
        out_h as u32,
        image::imageops::FilterType::Nearest,
    );

    let charset = CHARSETS[config.charset_index];
    let chars: Vec<char> = charset.chars().collect();
    let num_levels = chars.len();

    let n_pixels = out_w * out_h;

    let mut pixels = Vec::with_capacity(n_pixels);
    for y in 0..out_h {
        for x in 0..out_w {
            let c = resized.get_pixel(x as u32, y as u32);
            pixels.push((luminance(c[0], c[1], c[2]), c[0], c[1], c[2]));
        }
    }

    if config.contrast != 1.0 {
        for px in &mut pixels {
            let l = px.0 / 255.0;
            px.0 = (l.powf(config.contrast) * 255.0).clamp(0.0, 255.0);
        }
    }

    let mut luma_vals: Vec<f32> = pixels.iter().map(|p| p.0).collect();
    let edge_map = if config.edges {
        Some(sobel_edges_seq(&luma_vals, out_w, out_h, config.edge_threshold))
    } else {
        None
    };

    if config.dither {
        for y in 0..out_h {
            for x in 0..out_w {
                let idx = y * out_w + x;
                let old = luma_vals[idx];
                let level = (old / 255.0 * (num_levels - 1) as f32).round() as usize;
                let level = level.min(num_levels - 1);
                let new = level as f32 / (num_levels - 1) as f32 * 255.0;
                let err = old - new;
                luma_vals[idx] = new;

                if x + 1 < out_w {
                    luma_vals[y * out_w + x + 1] += err * 7.0 / 16.0;
                }
                if y + 1 < out_h {
                    if x > 0 {
                        luma_vals[(y + 1) * out_w + x - 1] += err * 3.0 / 16.0;
                    }
                    luma_vals[(y + 1) * out_w + x] += err * 5.0 / 16.0;
                    if x + 1 < out_w {
                        luma_vals[(y + 1) * out_w + x + 1] += err * 1.0 / 16.0;
                    }
                }
            }
        }
    }

    let invert = config.invert;
    let mut cells = Vec::with_capacity(out_h);
    for y in 0..out_h {
        cells.push(build_row(y, out_w, &pixels, &luma_vals, &edge_map, &chars, num_levels, invert));
    }

    Some(AsciiResult { cells, img_w, img_h, out_w, out_h })
}

#[cfg(not(target_arch = "wasm32"))]
fn sobel_edges_seq(gray: &[f32], w: usize, h: usize, threshold: f32) -> Vec<Option<usize>> {
    let mut edges = vec![None; w * h];
    let t2 = threshold * threshold;
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let base = (y - 1) * w + (x - 1);
            let mut gx = 0.0;
            let mut gy = 0.0;
            for ky in 0..3 {
                let r = &gray[base + ky * w..];
                gx += r[0] * -1.0 + r[2] * 1.0;
                gy += r[0] * -1.0 + r[1] * -2.0 + r[2] * -1.0;
            }
            let mag2 = gx * gx + gy * gy;
            if mag2 > t2 {
                let gy_abs = gy.abs();
                let gx_abs = gx.abs();
                let dir = if gy_abs > 2.414 * gx_abs {
                    0
                } else if gx_abs > 2.414 * gy_abs {
                    2
                } else if (gx > 0.0) == (gy > 0.0) {
                    3
                } else {
                    1
                };
                edges[y * w + x] = Some(dir);
            }
        }
    }
    edges
}

#[cfg(not(target_arch = "wasm32"))]
pub fn convert_video_frames(frames: &[DynamicImage], config: &AsciiConfig) -> Vec<AsciiResult> {
    let cfg = config.clone();
    frames
        .par_iter()
        .map(|frame| convert_frame(frame, &cfg).unwrap())
        .collect()
}

fn build_row(
    y: usize, out_w: usize, pixels: &[(f32, u8, u8, u8)],
    luma_vals: &[f32], edge_map: &Option<Vec<Option<usize>>>,
    chars: &[char], num_levels: usize, invert: bool,
) -> Vec<AsciiCell> {
    let mut row = Vec::with_capacity(out_w);
    for x in 0..out_w {
        let idx = y * out_w + x;
        let (_, r, g, b) = pixels[idx];
        let luma = luma_vals[idx].clamp(0.0, 255.0);

        if let Some(edges) = edge_map {
            if let Some(dir) = edges[idx] {
                row.push(AsciiCell {
                    ch: EDGE_CHARS[dir],
                    r,
                    g,
                    b,
                });
                continue;
            }
        }

        let level = ((luma / 255.0) * (num_levels - 1) as f32).round() as usize;
        let level = level.min(num_levels - 1);
        let ch = if invert {
            chars[num_levels - 1 - level]
        } else {
            chars[level]
        };
        row.push(AsciiCell { ch, r, g, b });
    }
    row
}

pub fn build_html(result: &AsciiResult, config: &AsciiConfig) -> String {
    use std::fmt::Write;

    let bg = if config.invert { "#fff" } else { "#000" };
    let fg = if config.invert { "#000" } else { "#fff" };

    let mut html = format!(
        r#"<!DOCTYPE html><html><head><meta charset="utf-8"><style>
body {{ background:{}; color:{}; font-family:'Courier New',Courier,monospace;
white-space:pre; line-height:1.05; font-size:10px; }}</style></head><body>
"#,
        bg, fg
    );

    for row in &result.cells {
        for cell in row {
            if config.color {
                let _ = write!(
                    html,
                    "<span style=\"color:#{:02x}{:02x}{:02x}\">{}</span>",
                    cell.r, cell.g, cell.b, cell.ch
                );
            } else {
                html.push(cell.ch);
            }
        }
        html.push('\n');
    }

    html.push_str("</body></html>");
    html
}
