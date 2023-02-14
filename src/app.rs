use crate::{
    utils::{Display, Stats},
    ParsedFile,
};
use anyhow::{bail, Context as _, Error, Result};
use eframe::{epaint::Hsva, get_value, set_value, CreationContext, Frame, Storage, APP_KEY};
use egui::{
    menu,
    plot::{Bar, BarChart, BoxElem, BoxPlot, BoxSpread, Legend, Plot, PlotPoint, Text},
    warn_if_debug_build, Align, Align2, CentralPanel, Color32, Context, DragValue, DroppedFile, Id,
    LayerId, Layout, Order, Response, SidePanel, TextStyle, TopBottomPanel, Ui, Window,
};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    default::default,
    fmt::Write,
    fs::read_to_string,
};

// macro_rules! normalize {
//     ($self:ident, $value:expr) => {{
//         let mut value = $value;
//         if $self.normalize {
//             value /= $self.max;
//             if $self.percent {
//                 value *= 100.0;
//             }
//         }
//         value
//     }};
// }
macro normalize($self:ident, $value:expr) {{
    let mut value = $value;
    if $self.normalize {
        value /= $self.max;
        if $self.percent {
            value *= 100.0;
        }
    }
    value
}}

macro unnormalize($self:ident, $value:expr) {{
    let mut value = $value;
    if $self.normalize {
        value *= $self.max;
        if $self.percent {
            value /= 100.0;
        }
    }
    value
}}

fn color(index: usize) -> Color32 {
    // 0.61803398875
    let golden_ratio: f32 = (5.0_f32.sqrt() - 1.0) / 2.0;
    let h = index as f32 * golden_ratio;
    Hsva::new(h, 0.85, 0.5, 1.0).into()
}

fn read(file: &DroppedFile) -> Result<String> {
    Ok(match &file.bytes {
        Some(bytes) => String::from_utf8(bytes.to_vec())?,
        None => match &file.path {
            Some(path) => read_to_string(&path)?,
            None => bail!("Dropped file hasn't bytes or path"),
        },
    })
}

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(Default, Deserialize, Serialize)]
#[serde(default)]
pub struct App {
    files: IndexMap<String, ParsedFile>,
    filter: HashSet<String>,
    filtered: Vec<usize>,

    plot_kind: PlotKind,
    normalize: bool,
    percent: bool,
    bound: f64,

    #[serde(skip)]
    errors: Errors,
    max: f64,
}

impl App {
    /// Called once before the first frame.
    pub fn new(cc: &CreationContext) -> Self {
        // This is also where you can customize the look and feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.

        // Load previous app state (if any).
        // Note that you must enable the `persistence` feature for this to work.
        if let Some(storage) = cc.storage {
            return get_value(storage, APP_KEY).unwrap_or_default();
        }
        default()
    }

    fn drag_and_drop_files(&mut self, ctx: &Context) {
        // Preview hovering files
        if let Some(text) = ctx.input(|input| {
            (!input.raw.hovered_files.is_empty()).then(|| {
                let mut text = String::from("Dropping files:");
                for file in &input.raw.hovered_files {
                    write!(text, "\n{}", file.display()).ok();
                }
                text
            })
        }) {
            let painter =
                ctx.layer_painter(LayerId::new(Order::Foreground, Id::new("file_drop_target")));
            let screen_rect = ctx.screen_rect();
            painter.rect_filled(screen_rect, 0.0, Color32::from_black_alpha(192));
            painter.text(
                screen_rect.center(),
                Align2::CENTER_CENTER,
                text,
                TextStyle::Heading.resolve(&ctx.style()),
                Color32::WHITE,
            );
        }
        // Parse dropped files
        if let Some(files) = ctx.input(|input| {
            (!input.raw.dropped_files.is_empty()).then_some(input.raw.dropped_files.clone())
        }) {
            for file in files {
                let key = file.display().to_string();
                let content = match read(&file) {
                    Ok(content) => content,
                    Err(error) => {
                        self.errors
                            .buffer
                            .insert(key, error.context("Read file error"));
                        continue;
                    }
                };
                let value = match content.parse() {
                    Ok(file) => file,
                    Err(error) => {
                        self.errors.buffer.insert(key, error);
                        continue;
                    }
                };
                self.files.insert(key.clone(), value);
            }
            self.update();
        }
    }

    fn bottom_panel(&mut self, ctx: &Context) {
        TopBottomPanel::bottom("bottom_panel").show(ctx, |ui| {
            menu::bar(ui, |ui| {
                ui.toggle_value(&mut self.errors.show, "⚠");
            });
        });
    }

    fn central_panel(&mut self, ctx: &Context) {
        CentralPanel::default().show(ctx, |ui| {
            if self.files.is_empty() {
                ui.centered_and_justified(|ui| ui.label("Drag and drop .msp file"))
                    .response
            } else {
                match self.files.len() {
                    1 => {
                        ui.heading(&self.files[0].name);
                    }
                    length => {
                        ui.heading(format!(
                            "{} ... {}",
                            self.files[0].name,
                            self.files[length - 1].name
                        ));
                    }
                }
                match self.plot_kind {
                    PlotKind::Box => self.box_plot(ui),
                    PlotKind::Chart => self.chart_plot(ui),
                    PlotKind::Difference => self.difference_plot(ui),
                }
            }
        });
    }

    fn left_panel(&mut self, ctx: &Context) {
        SidePanel::left("left_panel").show(ctx, |ui| {
            ui.heading("Left Panel");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.plot_kind, PlotKind::Box, "Box plot");
                ui.selectable_value(&mut self.plot_kind, PlotKind::Chart, "Chart plot");
                ui.selectable_value(&mut self.plot_kind, PlotKind::Difference, "Difference");
            });
            ui.horizontal(|ui| {
                if ui.toggle_value(&mut self.normalize, "Normalize").changed() {
                    if self.normalize {
                        self.bound /= self.max;
                    } else {
                        self.bound *= self.max;
                    }
                }
                if self.normalize {
                    if ui.toggle_value(&mut self.percent, "%").changed() {
                        if self.percent {
                            self.bound *= 100.0;
                        } else {
                            self.bound /= 100.0;
                        }
                    }
                }
            });
            ui.horizontal(|ui| {
                ui.label("Bound:");
                let end = if self.normalize {
                    if self.percent {
                        100
                    } else {
                        1
                    }
                } else {
                    u64::MAX
                };
                if ui
                    .add(
                        DragValue::new(&mut self.bound)
                            .clamp_range(0..=end)
                            .custom_formatter(|value, _| normalize!(self, value).to_string()),
                    )
                    .changed()
                {
                    // self.bound = self.unnormalize(normalized);
                }
            });
            ui.with_layout(Layout::bottom_up(Align::LEFT), |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    warn_if_debug_build(ui);
                    ui.label("powered by ");
                    ui.hyperlink_to("egui", "https://github.com/emilk/egui");
                    ui.label(" and ");
                    ui.hyperlink_to(
                        "eframe",
                        "https://github.com/emilk/egui/tree/master/crates/eframe",
                    );
                    ui.label(".");
                });
            });
        });
    }

    fn top_panel(&self, ctx: &Context, frame: &mut Frame) {
        #[cfg(not(target_arch = "wasm32"))] // no File->Quit on web pages!
        TopBottomPanel::top("top_panel").show(ctx, |ui| {
            // The top panel is often a good place for a menu bar:
            menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Quit").clicked() {
                        frame.close();
                    }
                });
            });
        });
    }

    fn errors(&mut self, ctx: &Context) {
        // Show errors
        Window::new("Errors")
            .open(&mut self.errors.show)
            .show(ctx, |ui| {
                if self.errors.buffer.is_empty() {
                    ui.label("No errors");
                } else {
                    self.errors.buffer.retain(|file, error| {
                        ui.horizontal(|ui| {
                            ui.label(file).on_hover_text(error.to_string());
                            !ui.button("🗙").clicked()
                        })
                        .inner
                    });
                }
            });
    }

    fn files(&mut self, ctx: &Context) {
        // Show files (if any):
        if !self.files.is_empty() {
            let mut open = true;
            Window::new("Files")
                .anchor(Align2::RIGHT_BOTTOM, [0.0, 0.0])
                .open(&mut open)
                .show(ctx, |ui| {
                    let mut changed = false;
                    self.files.retain(|key, _| {
                        ui.horizontal(|ui| {
                            let mut include = !self.filter.contains(key);
                            changed |= ui.checkbox(&mut include, "").changed();
                            if changed {
                                if include {
                                    self.filter.remove(key);
                                } else {
                                    self.filter.insert(key.clone());
                                }
                            }
                            ui.label(key);
                            let clicked = ui.button("🗙").clicked();
                            changed |= clicked;
                            !clicked
                        })
                        .inner
                    });
                    if changed {
                        self.update();
                    }
                });
            if !open {
                self.files.clear();
            }
        }
    }
}

impl App {
    fn update(&mut self) {
        self.filter();
        self.max();
    }

    fn filter(&mut self) {
        self.filtered = self
            .files
            .iter()
            .enumerate()
            .filter_map(|(index, (key, _))| (!self.filter.contains(key)).then_some(index))
            .collect();
    }

    fn max(&mut self) {
        self.max = self
            .filtered
            .iter()
            .flat_map(|&index| self.files[index].peaks.values())
            .max()
            .copied()
            .unwrap_or(1) as _;
    }

    fn box_plot(&self, ui: &mut Ui) -> Response {
        Plot::new("plot")
            .legend(Legend::default())
            .show(ui, |plot_ui| {
                let mut peaks = HashMap::<_, Vec<f64>>::new();
                for (&mass, &intensity) in self
                    .filtered
                    .iter()
                    .flat_map(|&index| &self.files[index].peaks)
                {
                    let intensity = normalize!(self, intensity as f64);
                    peaks.entry(mass).or_default().push(intensity);
                }
                peaks.retain(|_, value| value.len() > 1);
                let boxes = peaks
                    .into_iter()
                    .map(|(key, values)| {
                        let lower_whisker = values.min();
                        let (quartile1, median, quartile3) = values.quartiles();
                        let upper_whisker = values.max();
                        BoxElem::new(
                            key as _,
                            BoxSpread::new(
                                lower_whisker,
                                quartile1,
                                median,
                                quartile3,
                                upper_whisker,
                            ),
                        )
                        .name(key)
                    })
                    .collect();
                let box_plot = BoxPlot::new(boxes).name("Experiment A");
                plot_ui.box_plot(box_plot);
            })
            .response
    }

    fn chart_plot(&self, ui: &mut Ui) -> Response {
        Plot::new("plot")
            .legend(Legend::default())
            .show(ui, |plot_ui| {
                for &index in &self.filtered {
                    let file = &self.files[index];
                    let peaks = file.peaks.iter().map(|(&mass, &intensity)| {
                        let intensity = normalize!(self, intensity as f64);
                        (mass as f64, intensity)
                    });
                    let bars = peaks
                        .clone()
                        .map(|(mass, intensity)| Bar::new(mass, intensity).name(mass))
                        .collect();
                    let bar_chart =
                        BarChart::new(bars)
                            .name(&file.name)
                            .element_formatter(Box::new(|bar, _| {
                                format!("{} {}", bar.argument, bar.value)
                            }));
                    plot_ui.bar_chart(bar_chart);
                    for (mass, intensity) in peaks {
                        if intensity > self.bound {
                            let text = Text::new(
                                PlotPoint::new(mass, intensity),
                                format!("{mass}, {intensity}"),
                            )
                            .name("Labels")
                            .color(Color32::WHITE)
                            .highlight(true);
                            plot_ui.text(text);
                        }
                    }
                }
            })
            .response
    }

    fn difference_plot(&self, ui: &mut Ui) -> Response {
        Plot::new("plot")
            .legend(Legend::default())
            .show(ui, |plot_ui| {
                let mut peaks = HashMap::<_, f64>::new();
                for (&mass, &intensity) in self
                    .filtered
                    .iter()
                    .flat_map(|&index| &self.files[index].peaks)
                {
                    let intensity = normalize!(self, intensity as f64);
                    peaks
                        .entry(mass)
                        .and_modify(|value| *value -= intensity)
                        .or_insert(intensity);
                }
                let (positive, negative) = peaks
                    .iter()
                    .map(|(&mass, &intensity)| Bar::new(mass as _, intensity).name(mass))
                    .partition(|bar| bar.value.is_sign_positive());
                let bar_chart = BarChart::new(positive)
                    .name("Positive")
                    .element_formatter(Box::new(|bar, _| format!("{} {}", bar.argument, bar.value)))
                    .color(color(0));
                plot_ui.bar_chart(bar_chart);
                let bar_chart = BarChart::new(negative)
                    .name("Negative")
                    .element_formatter(Box::new(|bar, _| format!("{} {}", bar.argument, bar.value)))
                    .color(color(1));
                plot_ui.bar_chart(bar_chart);
            })
            .response
    }
}

impl eframe::App for App {
    /// Called by the frame work to save state before shutdown.
    fn save(&mut self, storage: &mut dyn Storage) {
        set_value(storage, APP_KEY, self);
    }

    fn update(&mut self, ctx: &Context, frame: &mut Frame) {
        self.top_panel(ctx, frame);
        self.bottom_panel(ctx);
        self.left_panel(ctx);
        self.central_panel(ctx);
        // self.windows(ctx);
        self.drag_and_drop_files(ctx);
        self.errors(ctx);
        self.files(ctx);
    }
}

// /// Filtered
// struct Filtered<T>(T);
// impl<'a, T: Iterator<Item = &'a IndexMap<u64, u64>> + Clone> Filtered<T> {
//     fn maximum(&self) -> f64 {
//         self.0
//             .clone()
//             .flat_map(|peaks| self.files[index].peaks.values())
//             .max()
//             .copied()
//             .unwrap_or(1) as _
//     }
// }
// impl<T: Iterator> Iterator for Filtered<T> {
//     type Item = T::Item;
//     fn next(&mut self) -> Option<Self::Item> {
//         self.0.next()
//     }
// }

/// Errors
#[derive(Default)]
struct Errors {
    show: bool,
    buffer: IndexMap<String, Error>,
}

/// Plot kind
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
enum PlotKind {
    Box,
    #[default]
    Chart,
    Difference,
}
