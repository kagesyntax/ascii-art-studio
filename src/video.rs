use image::DynamicImage;
use std::io::Read;
use std::process::{Command, Stdio};

pub struct VideoInfo {
    pub width: u32,
    pub height: u32,
    pub fps: f32,
    pub frame_count: usize,
    pub duration_secs: f32,
}

pub fn probe(path: &str) -> Option<VideoInfo> {
    let output = Command::new("ffprobe")
        .args([
            "-v", "quiet",
            "-select_streams", "v:0",
            "-show_entries", "stream=width,height,r_frame_rate,duration",
            "-of", "default=noprint_wrappers=1",
            path,
        ])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    let mut width = 0u32;
    let mut height = 0u32;
    let mut fps = 30.0f32;
    let mut duration = 0.0f32;
    for line in text.lines() {
        if let Some((key, val)) = line.split_once('=') {
            match key {
                "width" => width = val.parse().ok()?,
                "height" => height = val.parse().ok()?,
                "r_frame_rate" => {
                    if let Some((num, den)) = val.split_once('/') {
                        let n: f32 = num.parse().unwrap_or(30.0);
                        let d: f32 = den.parse().unwrap_or(1.0);
                        fps = n / d;
                    }
                }
                "duration" => duration = val.parse().ok()?,
                _ => {}
            }
        }
    }
    let frame_count = (duration * fps).round() as usize;
    Some(VideoInfo { width, height, fps, frame_count, duration_secs: duration })
}

pub struct FrameDecoder {
    child: Option<std::process::Child>,
    buf: Vec<u8>,
    frame_size: usize,
    pub info: VideoInfo,
}

impl FrameDecoder {
    pub fn open(path: &str, max_width: u32) -> Option<Self> {
        let info = probe(path)?;
        let scaled_w = if info.width > max_width { max_width } else { info.width };
        let ratio = scaled_w as f64 / info.width as f64;
        let scaled_h = (info.height as f64 * ratio).round() as u32;
        let frame_size = (scaled_w * scaled_h * 3) as usize;

        let child = Command::new("ffmpeg")
            .args([
                "-i", path,
                "-f", "rawvideo",
                "-pix_fmt", "rgb24",
                "-s", &format!("{}x{}", scaled_w, scaled_h),
                "-an", "-sn",
                "-",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;

        Some(FrameDecoder {
            child: Some(child),
            buf: Vec::with_capacity(frame_size),
            frame_size,
            info: VideoInfo {
                width: scaled_w,
                height: scaled_h,
                fps: info.fps,
                frame_count: info.frame_count,
                duration_secs: info.duration_secs,
            },
        })
    }

    pub fn next_frame(&mut self) -> Option<DynamicImage> {
        let child = self.child.as_mut()?;
        let stdout = child.stdout.as_mut()?;
        self.buf.clear();
        self.buf.resize(self.frame_size, 0);
        let mut read = 0;
        while read < self.frame_size {
            let n = stdout.read(&mut self.buf[read..]).ok()?;
            if n == 0 {
                return None;
            }
            read += n;
        }
        let img = image::RgbImage::from_raw(self.info.width, self.info.height, self.buf.clone())?;
        Some(DynamicImage::ImageRgb8(img))
    }
}

impl Drop for FrameDecoder {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
