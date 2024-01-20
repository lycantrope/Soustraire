use eframe::egui;
use eframe::egui::{widgets, CentralPanel, SidePanel, TopBottomPanel};
use egui::{FontFamily, FontId, TextStyle};
use poll_promise::Promise;
use rayon::prelude::*;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

mod font;
mod imagestack;
mod process;
mod roi;
mod toggle;

type Cache = Option<(usize, image::ImageBuffer<image::Rgba<u8>, Vec<u8>>)>;
/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
#[derive(Default)]
pub struct Subtractor {
    imagestack: imagestack::ImageStack<String>,
    roicol: roi::RoiCollection,

    show_subtract: bool,

    threshold: f64,
    #[serde(skip)]
    start: usize,
    #[serde(skip)]
    end: usize,
    #[serde(skip)]
    step: usize,

    #[serde(skip)]
    image: Option<imagestack::Image>,
    #[serde(skip)]
    processing: Option<Promise<()>>,

    #[serde(skip)]
    progress_total: usize,
    #[serde(skip)]
    progress_count: Arc<AtomicUsize>,

    #[serde(skip)]
    is_alive: bool,

    #[serde(skip)]
    cache: Arc<Cache>,
}

fn configure_text_styles(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "ROBOTO".to_owned(),
        egui::FontData::from_static(font::ROBOTO_FNT.as_ref()),
    );
    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, "ROBOTO".to_owned());
    let mut style = (*ctx.style()).clone();
    style.text_styles = [
        (
            TextStyle::Heading,
            FontId::new(25.0, FontFamily::Proportional),
        ),
        (TextStyle::Body, FontId::new(14.0, FontFamily::Proportional)),
        (
            TextStyle::Monospace,
            FontId::new(12.0, FontFamily::Monospace),
        ),
        (
            TextStyle::Button,
            FontId::new(16.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Small,
            FontId::new(10.0, FontFamily::Proportional),
        ),
    ]
    .into();
    ctx.set_style(style);
}

impl Subtractor {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // This is also where you can customize the look and feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.
        configure_text_styles(&cc.egui_ctx);

        // Load previous app state (if any).
        // Note that you must enable the `persistence` feature for this to work.
        if let Some(storage) = cc.storage {
            return eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default();
        }

        Default::default()
    }

    fn show_image(&mut self, ui: &mut egui::Ui) {
        match self.imagestack.get_current_images(self.step) {
            (None, None) => eprintln!("No image in stack"),
            (Some(im_path), None) => match self.cache.as_ref() {
                Some((pos, _)) if *pos == self.imagestack.pos => (),
                _ => {
                    let im = image::open(im_path).expect("fail to open image").to_rgba8();
                    self.cache = Arc::new(Some((self.imagestack.pos, im)));
                }
            },
            (pre, Some(im_path)) => match self.cache.as_ref() {
                Some((pos, _)) if *pos == self.imagestack.pos => (),
                _ => {
                    let im = match (self.show_subtract, pre) {
                        (true, Some(pre)) => {
                            let sub =
                                process::subtract(pre, im_path).expect("fail to to open image");

                            let thresh = (128.0f64 - self.threshold * 12.8f64)
                                .clamp(0f64, 255f64)
                                .round() as usize;
                            let mut im: image::ImageBuffer<image::Rgba<u8>, Vec<u8>> =
                                image::ImageBuffer::new(sub.width(), sub.height());
                            let mut rlut: [[u8; 4]; 256] = [[255; 4]; 256];
                            (0..=255u8).for_each(|val| rlut[val as usize] = [val, val, val, 255]);
                            (0..=thresh).for_each(|idx| rlut[idx][0] = 255);
                            im.chunks_exact_mut(4).zip(sub.iter().cloned()).for_each(
                                |(dst, src)| {
                                    dst.copy_from_slice(rlut[src as usize].as_slice());
                                },
                            );
                            im
                        }
                        (_, _) => image::open(im_path).expect("fail to open image").to_rgba8(),
                    };
                    self.cache = Arc::new(Some((self.imagestack.pos, im)));
                }
            },
        };

        if let Some((_, im)) = self.cache.as_ref() {
            let mut im = im.clone();
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

    fn spawn_a_process(&mut self) -> Promise<()> {
        let homedir = self
            .imagestack
            .homedir
            .as_ref()
            .expect("homedir should be not None while call this function");
        let mut roicol = self.roicol.clone();
        roicol.update_rois();

        let roi_path = Path::new(homedir).join("Roi.json");
        roicol.to_json(roi_path).expect("fail to write Roi.json");

        let csv_path = Path::new(homedir).join("Area.csv");
        let mut writer = csv::Writer::from_path(csv_path).expect("fail to create Area.csv");
        writer
            .write_record(&csv::StringRecord::from(vec!["Area"; roicol.len()]))
            .expect("fail to write csv header");

        let threshold = self.threshold;
        let _start = std::cmp::min(self.start, self.end).saturating_sub(self.step);
        let _end = std::cmp::max(self.end, self.start);
        let _step = self.step;

        let images = self
            .imagestack
            .get_stacks()
            .expect("the stacks are guarantee non-empty");

        let n_thread = num_cpus::get().saturating_sub(1).max(1);

        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(n_thread)
            .build()
            .expect("Fail to build rayon threadpool");

        self.progress_total = (_end - _start) / _step;
        self.progress_count.store(0, Ordering::SeqCst);
        let count = Arc::clone(&self.progress_count);

        poll_promise::Promise::spawn_thread("processing", move || {
            let res_sort = pool.install(|| {
                let mut res_sort: Vec<(usize, Vec<u32>)> = (_start.._end)
                    .into_par_iter()
                    .step_by(_step)
                    .enumerate()
                    // .with_min_len(chunksize)
                    .filter_map(|(pos, idx)| {
                        if let (Some(im1), Some(im2)) = (&images.get(idx), &images.get(idx + _step))
                        {
                            let subimg =
                                process::subtract(im1, im2).expect("failed to subtract the image");

                            let res = roicol
                                .measure_all(&subimg, threshold)
                                .expect("fail to measure Roi");
                            count.fetch_add(1, Ordering::SeqCst);
                            Some((pos, res))
                        } else {
                            None
                        }
                    })
                    .collect();
                res_sort.par_sort_unstable_by(|a, b| a.0.cmp(&b.0));
                res_sort
            });

            res_sort.into_iter().for_each(|(_, area)| {
                writer.serialize(area).expect("");
            });
            writer.flush().expect("fail to flush the writer");
        })
    }
}

impl eframe::App for Subtractor {
    /// Called by the frame work to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }
    fn warm_up_enabled(&self) -> bool {
        true
    }
    /// Called each time the UI needs repainting, which may be many times per second.
    /// Put your widgets into a `SidePanel`, `TopPanel`, `CentralPanel`, `Window` or `Area`.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        TopBottomPanel::top("slider").show(ctx, |ui| {
            ui.horizontal(|ui| {
                let max_frame = self.imagestack.max_slice();
                ui.label("Live");
                ui.add(toggle::toggle(&mut self.is_alive));
                let slider = widgets::Slider::new(&mut self.imagestack.pos, 0..=max_frame)
                    .show_value(true)
                    .text("pos")
                    .clamp_to_range(true)
                    .trailing_fill(true)
                    .step_by(1.0);

                if ui.add(slider).changed() {
                    self.show_image(ui);
                }
            });
        });

        TopBottomPanel::bottom("progress_bar").show(ctx, |ui| {
            let (pos, total) = if self.processing.is_some() {
                (
                    self.progress_count.load(Ordering::Relaxed),
                    self.progress_total,
                )
            } else {
                (self.imagestack.pos, self.imagestack.len())
            };
            ui.label(format!("{}/{}", pos, total));
            if let Some(promise) = self.processing.as_ref() {
                match promise.ready() {
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
                }
            }
        });

        SidePanel::left("control").show(ctx, |ui| {            
            ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                let proc_btn = egui::widgets::Button::new("Open Data Folder").min_size([128., 48.].into()).rounding(3.6);
                if self.processing.as_ref().is_none() && ui.add(proc_btn).clicked() {

                    let start_folder = self
                        .imagestack
                        .homedir
                        .as_ref()
                        .cloned()
                        .or_else(|| dirs::home_dir().map(|v| v.display().to_string()))
                        .expect("fail to set your home directory");

                    if let Some(path) = rfd::FileDialog::new()
                        .set_directory(start_folder)
                        .pick_folder()
                    {
                        self.imagestack.set_homedir(path.display().to_string());
                        match std::fs::File::open(path.join("Roi.json")){
                            Ok(fs) => {
                                let rdr = std::io::BufReader::new(fs);
                                self.roicol = serde_json::from_reader(rdr).unwrap_or_default();
                            }
                            Err(e) => eprintln!("json was not exists:{}", e),
                        }
                        self.start = 0;
                        self.end = self.imagestack.max_slice();
                        self.roicol.update_rois();
                        self.show_image(ui);
                    }
                }
            });

            let mut max_width = u32::MAX;
            let mut max_height = u32::MAX;
            if let Some(im) = self.image.as_ref() {
                let size = im.size;
                max_width = size[0] as u32;
                max_height = size[1] as u32;
            }

            ui.separator();
            // roicol collections
            ui.label("Parameters of Region of Interest(ROI)");
            ui.add_space(6.);
            let roi_labels: [&'static str; 9] = [
                "Number of Column",
                "Number of Row",
                "X Coordinate",
                "Y Coordinate",
                "X Interval of ROI",
                "Y Interval of ROI",
                "ROI Width",
                "ROI Height",
                "Rotate",
            ];
            let rois_widgets: Vec<widgets::DragValue<'_>> = vec![
                widgets::DragValue::new(&mut self.roicol.ncol).suffix(" cols").clamp_range(0..=100),
                widgets::DragValue::new(&mut self.roicol.nrow).suffix(" rows").clamp_range(0..=100),
                widgets::DragValue::new(&mut self.roicol.x).suffix(" px").clamp_range(0..=max_width),
                widgets::DragValue::new(&mut self.roicol.y).suffix(" px").clamp_range(0..=max_height),
                widgets::DragValue::new(&mut self.roicol.xinterval).suffix(" px").clamp_range(0..=max_width),
                widgets::DragValue::new(&mut self.roicol.yinterval).suffix(" px").clamp_range(0..=max_height),
                widgets::DragValue::new(&mut self.roicol.width).suffix(" px").clamp_range(0..=max_width),
                widgets::DragValue::new(&mut self.roicol.height).suffix(" px").clamp_range(0..=max_height),
                widgets::DragValue::new(&mut self.roicol.rotate).suffix(" °").clamp_range((-90.)..=90.),
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
                ctx.request_repaint();
            }

            if ui
                .add(widgets::Checkbox::new(
                    &mut self.show_subtract,
                    "show subtract",
                ))
                .changed(){
                self.cache = Arc::new(None);
                self.show_image(ui);
                ctx.request_repaint();
            }

            // show subtract
            let maxslice = self.imagestack.max_slice();
            ui.separator();
            ui.label("Binarized threshold (default: 2.0 x std)");
            if ui.add(
                widgets::DragValue::new(&mut self.threshold)
                    .min_decimals(1)
                    .clamp_range(-10.0..=10.0),
            ).changed(){
                self.cache = Arc::new(None);
                self.show_image(ui);
                ctx.request_repaint();
            };
            ui.label("Start slice");
            ui.add(
                widgets::DragValue::new(&mut self.start)
                    .clamp_range(0..=maxslice),
            );
            ui.label("End slice");
            ui.add(
                widgets::DragValue::new(&mut self.end).clamp_range(0..=maxslice),
            );

            ui.label("Frame step");
            if ui.add(
                widgets::DragValue::new(&mut self.step).clamp_range(1..=maxslice.saturating_sub(1).max(1)),
            ).changed(){
                self.cache = Arc::new(None);
                self.show_image(ui);
                ctx.request_repaint();
            };

            // process block
            ui.separator();
            ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                if let Some(homedir) = &self.imagestack.homedir {
                    if self.processing.is_some() {
                        ui.label(format!("Processing the data in: {}", homedir.to_owned()));
                    } else if self.imagestack.max_slice() <= self.step{
                        ui.label("Cannot processing the imagestack if step size is greater than total frame!!");
                    } else if ui.add(widgets::Button::new("Start Process").min_size([128., 48.].into()).rounding(3.6)).clicked() {
                        let promise = self.spawn_a_process();
                        self.processing = Some(promise);
                    }
                }
            });
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
                let total = self.imagestack.len();
                let pos = self.imagestack.pos;
                if self.is_alive {
                    self.imagestack.pos = (pos + 1) % total;
                }
                if response.clicked_by(egui::PointerButton::Primary) {
                    self.imagestack.pos = (pos + total - self.step) % total;
                }
                if response.clicked_by(egui::PointerButton::Secondary) {
                    self.imagestack.pos = (pos + self.step) % total;
                }
                if self.imagestack.pos != pos {
                    self.show_image(ui);
                    ctx.request_repaint();
                }
            };
        });
    }
}
