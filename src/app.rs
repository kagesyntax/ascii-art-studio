use crate::engine::{self, AsciiConfig, AsciiResult};
use eframe::egui::{self, FontFamily, FontId};
use image::DynamicImage;

pub struct AsciiApp {
    pub image: Option<DynamicImage>,
    pub result: Option<AsciiResult>,
    pub config: AsciiConfig,
    pub dirty: bool,
    pending_file: Option<poll_promise::Promise<Option<Vec<u8>>>>,
}

impl Default for AsciiApp {
    fn default() -> Self {
        Self {
            image: None,
            result: None,
            config: AsciiConfig::default(),
            dirty: false,
            pending_file: None,
        }
    }
}

impl AsciiApp {
    pub fn load_image_bytes(&mut self, bytes: &[u8]) {
        match image::load_from_memory(bytes) {
            Ok(img) => {
                self.image = Some(img);
                self.dirty = true;
            }
            Err(e) => {
                eprintln!("Failed to load image: {}", e);
            }
        }
    }

    pub fn recompute(&mut self) {
        if let Some(ref img) = self.image {
            self.result = engine::convert_to_ascii(img, &self.config);
        }
        self.dirty = false;
    }

    fn button_open(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.pending_file = Some(poll_promise::Promise::spawn_thread("file_open", || {
                let file = pollster::block_on(
                    rfd::AsyncFileDialog::new()
                        .add_filter(
                            "Images",
                            &["png", "jpg", "jpeg", "gif", "bmp", "webp", "tiff"],
                        )
                        .pick_file(),
                );
                Some(std::fs::read(file?.path()).ok()?)
            }));
        }

        #[cfg(target_arch = "wasm32")]
        self.button_open_wasm();
    }

    #[cfg(target_arch = "wasm32")]
    fn button_open_wasm(&mut self) {
        use wasm_bindgen::closure::Closure;
        use wasm_bindgen::JsCast;
        use wasm_bindgen::JsValue;
        use wasm_bindgen_futures::JsFuture;

        self.pending_file = Some(poll_promise::Promise::spawn_local(async {
            let window = web_sys::window()?;
            let document = window.document()?;

            let input: web_sys::HtmlInputElement = document
                .create_element("input")
                .ok()?
                .dyn_into::<web_sys::HtmlInputElement>()
                .ok()?;
            input.set_type("file");
            input.set_accept(
                "image/png,image/jpeg,image/gif,image/webp,image/bmp,image/tiff",
            );
            let _ = input.style().set_css_text("display:none");
            document.body()?.append_child(input.as_ref()).ok()?;

            let promise = js_sys::Promise::new(&mut |resolve, _reject| {
                let input2 = input.clone();
                let r = resolve.clone();
                let cb = Closure::once(move || {
                    if let Some(file) = input2.files().and_then(|fl| fl.get(0)) {
                        let reader = web_sys::FileReader::new().unwrap();
                        let reader2 = reader.clone();
                        let r2 = r.clone();
                        let onload = Closure::once(move || {
                            let val = reader
                                .result()
                                .ok()
                                .unwrap_or(JsValue::NULL);
                            let _ = r2.call1(&JsValue::UNDEFINED, &val);
                        });
                        let _ = reader2.set_onload(Some(onload.as_ref().unchecked_ref()));
                        onload.forget();
                        let blob: &web_sys::Blob = file.unchecked_ref();
                        let _ = reader2.read_as_array_buffer(blob);
                    } else {
                        let _ = r.call1(&JsValue::UNDEFINED, &JsValue::NULL);
                    }
                    if let Some(parent) = input2.parent_node() {
                        let _ = parent.remove_child(&input2);
                    }
                });
                let _ = input.add_event_listener_with_callback(
                    "change",
                    cb.as_ref().unchecked_ref(),
                );
                cb.forget();
                input.click();
            });

            let val = JsFuture::from(promise).await.ok()?;
            if val.is_null() || val.is_undefined() {
                return None;
            }
            let buf = js_sys::Uint8Array::new(&val);
            Some(buf.to_vec())
        }));
    }

    fn button_save_text(&self) {
        if let Some(ref result) = self.result {
            let text: String = result
                .cells
                .iter()
                .map(|row| row.iter().map(|c| c.ch).collect::<String>())
                .collect::<Vec<_>>()
                .join("\n");

            #[cfg(not(target_arch = "wasm32"))]
            std::thread::spawn(move || {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Text", &["txt"])
                    .set_file_name("ascii.txt")
                    .save_file()
                {
                    let _ = std::fs::write(&path, text);
                }
            });

            #[cfg(target_arch = "wasm32")]
            download_file("ascii.txt", &text, "text/plain");
        }
    }

    fn button_save_html(&self) {
        if let Some(ref result) = self.result {
            let html = engine::build_html(result, &self.config);

            #[cfg(not(target_arch = "wasm32"))]
            std::thread::spawn(move || {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("HTML", &["html"])
                    .set_file_name("ascii.html")
                    .save_file()
                {
                    let _ = std::fs::write(&path, html);
                }
            });

            #[cfg(target_arch = "wasm32")]
            download_file("ascii.html", &html, "text/html");
        }
    }

    fn handle_dropped_files(&mut self, files: &[egui::DroppedFile]) {
        for file in files {
            #[cfg(not(target_arch = "wasm32"))]
            if let Some(ref path) = file.path {
                if let Some(img) = try_load_path(path) {
                    self.image = Some(img);
                    self.dirty = true;
                }
            }

            #[cfg(target_arch = "wasm32")]
            if let Some(bytes) = &file.bytes {
                self.load_image_bytes(bytes);
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn try_load_path(path: &std::path::Path) -> Option<DynamicImage> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    match ext.to_lowercase().as_str() {
        "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp" | "tiff" | "tif" => {
            image::open(path).ok()
        }
        _ => None,
    }
}

#[cfg(target_arch = "wasm32")]
fn download_file(name: &str, content: &str, mime: &str) {
    let win = web_sys::window().expect("no window");
    let doc = win.document().expect("no document");
    let a = doc.create_element("a").expect("create a");
    let _ = a.set_attribute("download", name);
    let opts = web_sys::BlobPropertyBag::new();
    opts.set_type(mime);
    let blob = web_sys::Blob::new_with_str_sequence_and_options(
        &wasm_bindgen::JsValue::from_str(content).into(),
        &opts,
    )
    .expect("blob");
    let url = web_sys::Url::create_object_url_with_blob(&blob).expect("url");
    let _ = a.set_attribute("href", &url);
    let _ = doc.body().unwrap().append_child(&a);
    let _ = a.dispatch_event(&web_sys::MouseEvent::new("click").expect("click"));
    let _ = doc.body().unwrap().remove_child(&a);
    let _ = web_sys::Url::revoke_object_url(&url);
}

impl eframe::App for AsciiApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        {
            let done = self.pending_file.as_ref().and_then(|p| p.ready().cloned());
            if let Some(Some(bytes)) = done {
                self.load_image_bytes(&bytes);
                self.pending_file = None;
            }
        }

        if self.dirty {
            self.recompute();
        }

        egui::Panel::top("controls").show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 4.0;
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                ui.spacing_mut().item_spacing.y = 4.0;

                if ui.button("Open Image").clicked() {
                    self.button_open();
                }

                if self.result.is_some() && ui.button("Save Text").clicked() {
                    self.button_save_text();
                }

                if self.result.is_some() && ui.button("Save HTML").clicked() {
                    self.button_save_html();
                }

                if let Some(ref result) = self.result {
                    ui.separator();
                    ui.label(format!(
                        "{}×{} px → {}×{} chars",
                        result.img_w, result.img_h, result.out_w, result.out_h,
                    ));
                }

                ui.separator();

                ui.label("Width:");
                let mut w = self.config.width_chars as f32;
                if ui
                    .add(egui::Slider::new(&mut w, 20.0..=400.0).integer())
                    .changed()
                {
                    self.config.width_chars = w as usize;
                    self.dirty = true;
                }

                ui.separator();

                if ui.checkbox(&mut self.config.color, "Color").changed() {
                    self.dirty = true;
                }
                if ui.checkbox(&mut self.config.dither, "Dither").changed() {
                    self.dirty = true;
                }
                if ui.checkbox(&mut self.config.edges, "Edges").changed() {
                    self.dirty = true;
                }
                if ui.checkbox(&mut self.config.invert, "Invert").changed() {
                    self.dirty = true;
                }

                ui.separator();

                ui.label("Contrast:");
                if ui
                    .add(
                        egui::Slider::new(&mut self.config.contrast, 0.2..=3.0)
                            .logarithmic(true),
                    )
                    .changed()
                {
                    self.dirty = true;
                }

                if self.config.edges {
                    ui.label("Edge:");
                    if ui
                        .add(
                            egui::Slider::new(
                                &mut self.config.edge_threshold,
                                1.0..=200.0,
                            )
                            .logarithmic(true),
                        )
                        .changed()
                    {
                        self.dirty = true;
                    }
                }

                ui.separator();

                ui.label("Font:");
                if ui
                    .add(egui::Slider::new(
                        &mut self.config.font_size,
                        4.0..=24.0,
                    ))
                    .changed()
                {
                    self.dirty = true;
                }

                ui.separator();

                egui::ComboBox::from_id_salt("charset")
                    .selected_text(engine::CHARSET_NAMES[self.config.charset_index])
                    .show_ui(ui, |ui| {
                        for (i, name) in engine::CHARSET_NAMES.iter().enumerate() {
                            if ui
                                .selectable_label(self.config.charset_index == i, *name)
                                .clicked()
                            {
                                self.config.charset_index = i;
                                self.dirty = true;
                            }
                        }
                    });
            });
        });

        let bg = if self.config.invert {
            egui::Color32::WHITE
        } else {
            egui::Color32::BLACK
        };

        egui::CentralPanel::default()
            .frame(egui::Frame::default().fill(bg))
            .show(ui, |ui| {
                if let Some(ref result) = self.result {
                    let font_size = self.config.font_size;
                    let font_id = FontId::new(font_size, FontFamily::Monospace);
                    let line_h = font_size * 1.15;
                    let char_w = font_size * 0.6;

                    let area_w = result.out_w as f32 * char_w + 8.0;
                    let area_h = result.out_h as f32 * line_h + 8.0;
                    ui.set_min_size(egui::vec2(area_w, area_h));

                    let painter = ui.painter();
                    let fg = if self.config.invert {
                        egui::Color32::BLACK
                    } else {
                        egui::Color32::WHITE
                    };
                    let mut buf = [0u8; 4];

                    for (i, row) in result.cells.iter().enumerate() {
                        let y = 4.0 + i as f32 * line_h;

                        let mut job = egui::text::LayoutJob::default();
                        job.wrap.max_width = f32::INFINITY;
                        if self.config.color {
                            for cell in row {
                                let s = cell.ch.encode_utf8(&mut buf);
                                let fmt = egui::TextFormat {
                                    font_id: font_id.clone(),
                                    color: egui::Color32::from_rgb(cell.r, cell.g, cell.b),
                                    ..Default::default()
                                };
                                job.append(s, 0.0, fmt);
                            }
                        } else {
                            for cell in row {
                                let s = cell.ch.encode_utf8(&mut buf);
                                let fmt = egui::TextFormat {
                                    font_id: font_id.clone(),
                                    color: fg,
                                    ..Default::default()
                                };
                                job.append(s, 0.0, fmt);
                            }
                        }
                        painter.galley(egui::pos2(4.0, y), painter.layout_job(job), fg);
                    }
                } else {
                    ui.vertical_centered_justified(|ui| {
                        ui.heading("ASCII Art Studio");
                        ui.label("Open an image or drag & drop one here.");
                    });
                }
            });

        ui.input_mut(|i| {
            if i.consume_key(egui::Modifiers::COMMAND, egui::Key::O) {
                self.button_open();
            }
        });

        ui.input_mut(|i| {
            if !i.raw.dropped_files.is_empty() {
                self.handle_dropped_files(&i.raw.dropped_files);
                i.raw.dropped_files.clear();
            }
        });
    }
}
