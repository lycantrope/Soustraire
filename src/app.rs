use eframe::egui;
use eframe::egui::{widgets, CentralPanel, SidePanel, TopBottomPanel};
use poll_promise::Promise;
use rayon::prelude::*;

mod core;
mod font;
mod imagestack;
mod roi;

#[cfg(target_arch = "wasm32")]
use pollster::FutureExt as _;

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
#[derive(Default)]
pub struct Subtractor {
    imagestack: imagestack::ImageStack<String>,
    picked_path: Option<String>,
    roicol: roi::RoiCollection,

    show_subtract: bool,

    threshold: f64,
    start: usize,
    end: usize,

    #[serde(skip)]
    image: Option<imagestack::Image>,
    #[serde(skip)]
    processing: Option<Promise<()>>,
    #[serde(skip)]
    progress_rx: Option<std::sync::mpsc::Receiver<(usize, usize)>>,
    counter: usize,
}

impl Subtractor {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // This is also where you can customize the look and feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.

        // Load previous app state (if any).
        // Note that you must enable the `persistence` feature for this to work.
        if let Some(storage) = cc.storage {
            return eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default();
        }

        Default::default()
    }

    fn get_progress(&mut self) -> (usize, usize) {
        let default = (self.imagestack.pos, self.imagestack.len());
        if let Some(rx) = &self.progress_rx {
            if let Ok((_pos, total)) = rx.recv() {
                self.counter += 1;
                (self.counter, total)
            } else {
                self.counter = default.0;
                default
            }
        } else {
            self.counter = default.0;
            default
        }
    }
    fn show_image(&mut self, ui: &mut egui::Ui) {
        match self.imagestack.get_current_images() {
            (None, None) => eprintln!("No image in stack"),
            (Some(im_path), None) => {
                let mut im = image::open(im_path).expect("fail to open image").to_rgba8();
                self.roicol.draw_rois(&mut im);
                let size = [im.width() as usize, im.height() as usize];
                let texture = ui.ctx().load_texture(
                    format!("{}", self.imagestack.pos),
                    egui::ColorImage::from_rgba_unmultiplied(size, im.to_vec().as_slice()),
                    Default::default(),
                );

                self.image.replace(imagestack::Image {
                    size,
                    texture_id: Some(texture),
                });
            }
            (pre, Some(im_path)) => {
                let mut im = match (self.show_subtract, pre) {
                    (true, Some(pre)) => {
                        let sub = core::subtract(pre, im_path).expect("fail to to open image");

                        let thresh = (128.0f64 - self.threshold * 12.8f64)
                            .clamp(0f64, 255f64)
                            .round() as u8;
                        let thres_sub = imageproc::contrast::threshold(&sub, thresh);
                        let mut im: image::ImageBuffer<image::Rgba<u8>, Vec<u8>> =
                            image::ImageBuffer::new(sub.width(), sub.height());

                        im.par_chunks_mut(4)
                            .zip(
                                sub.into_par_iter()
                                    .cloned()
                                    .zip(thres_sub.into_par_iter().cloned()),
                            )
                            .for_each(|(dst, (src1, src2))| {
                                dst[0] = src1 | (!src2);
                                dst[1] = src1;
                                dst[2] = src1;
                                dst[3] = 255
                            });
                        // im.pixels_mut().zip(sub.pixels()).for_each(|(dst, src)| {
                        //     let image::Luma([val]) = src;
                        //     *dst = image::Rgba([*val, *val, *val, 255]);
                        // });
                        im
                    }
                    (_, _) => image::open(im_path).expect("fail to open image").to_rgba8(),
                };

                self.roicol.draw_rois(&mut im);
                let size = [im.width() as usize, im.height() as usize];
                let texture = ui.ctx().load_texture(
                    format!("{}", self.imagestack.pos),
                    egui::ColorImage::from_rgba_unmultiplied(size, im.to_vec().as_slice()),
                    Default::default(),
                );

                self.image.replace(imagestack::Image {
                    size,
                    texture_id: Some(texture),
                });
            }
        }
    }
}

impl eframe::App for Subtractor {
    /// Called by the frame work to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    /// Put your widgets into a `SidePanel`, `TopPanel`, `CentralPanel`, `Window` or `Area`.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let max_frame = self.imagestack.len().checked_sub(1).unwrap_or_default();
        TopBottomPanel::top("slider").show(ctx, |ui| {
            let slider = widgets::Slider::new(&mut self.imagestack.pos, 0..=max_frame)
                .text("pos")
                .clamp_to_range(true)
                .trailing_fill(true);
            if ui.add(slider).changed() {
                self.show_image(ui);
            }
        });

        TopBottomPanel::bottom("progress_bar").show(ctx, |ui| {
            let (pos, total) = self.get_progress();
            ui.label(format!("{}/{}", pos, total));
            match self.processing.as_ref() {
                None => {}
                Some(promise) => match promise.ready() {
                    None => {
                        let progress = pos as f32 / total as f32;
                        let progress_bar = egui::ProgressBar::new(progress)
                            .show_percentage()
                            .animate(true);
                        ui.add(progress_bar);
                    }
                    Some(_) => {
                        self.processing.take();
                    }
                },
            }
        });

        SidePanel::left("control").show(ctx, |ui| {
            if ui.button("Open").clicked() {
                #[cfg(not(target_arch = "wasm32"))]
                let start_folder = self
                    .imagestack
                    .homedir
                    .as_ref()
                    .map(|p| p.to_string())
                    .unwrap_or(
                        dirs::home_dir()
                            .expect("fail to find your home directory")
                            .display()
                            .to_string(),
                    );
                if let Some(path) = rfd::FileDialog::new()
                    .set_directory(start_folder)
                    .pick_folder()
                {
                    self.picked_path = Some(path.display().to_string());
                    self.imagestack.set_homedir(path.display().to_string());
                    match std::fs::File::open(path.join("Roi.json")) {
                        Ok(fs) => {
                            let rdr = std::io::BufReader::new(fs);
                            self.roicol =
                                serde_json::from_reader(rdr).expect("fail to parse the json file");
                        }
                        Err(e) => eprintln!("json was not exists:{}", e),
                    }
                    self.roicol.update_rois();
                }
                #[cfg(target_arch = "wasm32")]
                {
                    let f = async { rfd::AsyncFileDialog::new().pick_file().await };
                    if let Some(path) = f.block_on() {
                        self.picked_path = path.path().parent();
                        self.picked_path = Some(path.display().to_string());
                        self.imagestack.set_homedir(path.display().to_string());
                        match std::fs::File::open(path.join("Roi.json")) {
                            Ok(fs) => {
                                let rdr = std::io::BufReader::new(fs);
                                self.roicol = serde_json::from_reader(rdr)
                                    .expect("fail to parse the json file");
                            }
                            Err(e) => eprintln!("json was not exists:{}", e),
                        }
                        self.roicol.update_rois();
                    }
                }

                // self.show_image(ui);
                self.show_image(ui);
            }

            let mut max_width = u32::MAX;
            let mut max_height = u32::MAX;
            if let Some(im) = self.image.as_ref() {
                let size = im.size;
                max_width = size[0] as u32;
                max_height = size[1] as u32;
            }

            ui.separator();
            // roicol collections
            ui.label("Roi manipulation");
            let roi_labels: [&'static str; 9] = [
                "ncol",
                "nrow",
                "x",
                "y",
                "xinterval",
                "yinterval",
                "width",
                "height",
                "rotate",
            ];
            let rois_widgets: Vec<widgets::DragValue<'_>> = vec![
                widgets::DragValue::new(&mut self.roicol.ncol).clamp_range(0..=100),
                widgets::DragValue::new(&mut self.roicol.nrow).clamp_range(0..=100),
                widgets::DragValue::new(&mut self.roicol.x).clamp_range(0..=max_width),
                widgets::DragValue::new(&mut self.roicol.y).clamp_range(0..=max_height),
                widgets::DragValue::new(&mut self.roicol.xinterval).clamp_range(0..=max_width),
                widgets::DragValue::new(&mut self.roicol.yinterval).clamp_range(0..=max_height),
                widgets::DragValue::new(&mut self.roicol.width).clamp_range(0..=max_width),
                widgets::DragValue::new(&mut self.roicol.height).clamp_range(0..=max_height),
                widgets::DragValue::new(&mut self.roicol.rotate)
                    .clamp_range((-90.)..=90.)
                    .min_decimals(1),
            ];

            if roi_labels.into_iter().zip(rois_widgets.into_iter()).fold(
                false,
                |changed, (label, widget)| {
                    ui.label(label);
                    changed | ui.add(widget).changed()
                },
            ) {
                self.roicol.update_rois();
                self.show_image(ui);
            }
            if ui
                .add(widgets::Checkbox::new(
                    &mut self.show_subtract,
                    "show subtract",
                ))
                .changed()
            {
                self.show_image(ui);
            }

            // show subtract
            ui.separator();
            ui.label("binarized threshold (/per std)");
            ui.add(
                widgets::DragValue::new(&mut self.threshold)
                    .min_decimals(1)
                    .clamp_range(-10.0..=10.0),
            );
            ui.add(widgets::DragValue::new(&mut self.start).clamp_range(0..=self.end));
            ui.add(widgets::DragValue::new(&mut self.end).clamp_range(self.start..=max_frame));

            // process block
            if self.processing.is_none() {
                ui.separator();
                if ui.button("Start Process").clicked() {
                    if let Some(homedir) = &self.imagestack.homedir {
                        self.roicol.update_rois();
                        let roi_path = std::path::Path::new(homedir).join("Roi.json");
                        self.roicol
                            .to_json(roi_path.clone())
                            .expect("fail to write Roi");

                        match self.processing.as_ref() {
                            None => {
                                self.counter = 0;
                                let home = homedir.to_string();
                                let (tx, rx) = std::sync::mpsc::channel();
                                self.progress_rx.replace(rx);
                                let threshold = self.threshold;
                                let _start = self.start;
                                let _end = self.end;
                                let promise =
                                    poll_promise::Promise::spawn_thread("processing", move || {
                                        let mut stack = imagestack::ImageStack::<String>::default();
                                        stack.set_homedir(home.to_string());
                                        let csv_path = std::path::Path::new(&home).join("Area.csv");
                                        let roi_reader = std::fs::File::open(roi_path)
                                            .expect("Fail to read rois");
                                        let mut roicol: roi::RoiCollection =
                                            serde_json::from_reader(roi_reader)
                                                .expect("Fail to parser RoiCollections");
                                        roicol.update_rois();
                                        let mut writer = csv::Writer::from_path(csv_path)
                                            .expect("fail to create file");
                                        writer
                                            .write_record(&csv::StringRecord::from(vec![
                                                "Area";
                                                roicol
                                                    .len()
                                            ]))
                                            .expect("fail to write csv");
                                        let n = stack.len();
                                        let pool = rayon::ThreadPoolBuilder::new()
                                            .num_threads(num_cpus::get() / 2)
                                            .build()
                                            .expect("fail to create rayon pool");

                                        let res_sort = pool.install(|| {
                                            let mut res: Vec<(usize, Vec<u32>)> = stack
                                                .stacks
                                                .par_windows(2)
                                                .enumerate()
                                                .into_par_iter()
                                                .map_with(tx, |tx, (pos, ims)| {
                                                    let im1_p = &ims[0];
                                                    let im2_p = &ims[1];
                                                    let subimg = core::subtract(im1_p, im2_p)
                                                        .expect("failed to subtract the image");

                                                    let res = roicol
                                                        .measure_all(&subimg, threshold)
                                                        .expect("fail to measure Roi");

                                                    loop {
                                                        if let Ok(()) = tx.send((pos, n)) {
                                                            break;
                                                        };
                                                    }
                                                    (pos, res)
                                                })
                                                .collect();
                                            res.par_sort_unstable_by(|a, b| a.0.cmp(&b.0));
                                            res
                                        });

                                        res_sort.into_iter().for_each(|record| {
                                            writer.serialize(record.1).expect("");
                                        });
                                        writer.flush().expect("fail to flush the writer");
                                    });
                                self.processing = Some(promise);
                            }
                            Some(_) => (),
                        }
                    }
                }
            }
        });
        CentralPanel::default().show(ctx, |ui| {
            if let Some(im) = &mut self.image {
                let texture: &egui::TextureHandle = im.texture_id.get_or_insert_with(|| {
                    // Load the texture only once.
                    ui.ctx().load_texture(
                        "no image",
                        egui::ColorImage::example(),
                        Default::default(),
                    )
                });
                let response = ui.add(widgets::ImageButton::new(texture, texture.size_vec2()));
                if response.clicked_by(egui::PointerButton::Primary) {
                    let total = self.imagestack.stacks.len();
                    self.imagestack.pos = self
                        .imagestack
                        .pos
                        .checked_sub(1)
                        .unwrap_or(total.saturating_sub(1));
                }
                if response.clicked_by(egui::PointerButton::Secondary) {
                    let total = self.imagestack.stacks.len();
                    self.imagestack.pos = self.imagestack.pos.saturating_add(1) % total;
                }
                self.show_image(ui);
            };
        });
    }
}
