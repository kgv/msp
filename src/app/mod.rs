use self::{
    bounder::Bounded,
    predictioner::{Key, Predicted},
};
use crate::{
    parser::Parsed,
    utils::{with_index, Display, DroppedFileExt, RangeBoundsExt, UiExt},
};
use anyhow::Error;
use bitflags::bitflags;
use eframe::{epaint::Hsva, get_value, set_value, CreationContext, Frame, Storage, APP_KEY};
use egui::{
    global_dark_light_mode_switch,
    menu::bar,
    plot::{
        self, Bar, BarChart, CoordinatesFormatter, Corner, HLine, Legend, MarkerShape, Plot,
        PlotPoint, Points, Text, VLine,
    },
    warn_if_debug_build, Align, Align2, CentralPanel, Color32, Context, DragValue, DroppedFile, Id,
    LayerId, Layout, Order, Response, RichText, SidePanel, Slider, TextStyle, TopBottomPanel, Ui,
    WidgetText, Window,
};
use indexmap::IndexMap;
use ndarray::{Array1, Dimension};
use ndarray_stats::{interpolate::Linear, Quantile1dExt};
use noisy_float::types::n64;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fmt::{self, Write},
    ops::{Bound, RangeBounds},
};
use tracing::{error, info};

pub fn color(index: usize) -> Color32 {
    let golden_ratio: f32 = (5.0_f32.sqrt() - 1.0) / 2.0; // 0.61803398875
    let h = index as f32 * golden_ratio;
    Hsva::new(h, 0.85, 0.5, 1.0).into()
}

#[derive(Default, Deserialize, Serialize)]
#[serde(default)]
pub struct App {
    #[serde(skip)]
    files: Vec<DroppedFile>,
    parsed: HashMap<usize, Parsed>,
    colors: IndexMap<usize, Color32>,
    filter: HashSet<usize>,

    left_panel: bool,
    label: Label,

    // Filter
    bounds: Bounds,
    limits: Limits,

    // Find
    mass: usize,
    pattern: Vec<Vec<usize>>,
    count: usize,

    // Statistics
    statistics: Statistics,

    // Peak Finder
    lag: usize,
    threshold: f64,
    influence: f64,
    temp: f64,

    #[serde(skip)]
    errors: Errors,
}

impl App {
    /// Called once before the first frame.
    pub fn new(cc: &CreationContext) -> Self {
        // Customize style of egui.
        let mut style = (*cc.egui_ctx.style()).clone();
        style.visuals.collapsing_header_frame = true;
        cc.egui_ctx.set_style(style);
        // Load previous app state (if any).
        // Note that you must enable the `persistence` feature for this to work.
        cc.storage
            .and_then(|storage| get_value(storage, APP_KEY))
            .unwrap_or_default()
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
            info!(?files);
            self.files = files;
            for (index, file) in self.files.iter().enumerate() {
                let content = match file.content() {
                    Ok(content) => content,
                    Err(error) => {
                        error!(%error);
                        self.errors.buffer.insert(index, error);
                        continue;
                    }
                };
                let parsed = match content.parse() {
                    Ok(file) => file,
                    Err(error) => {
                        error!(%error);
                        self.errors.buffer.insert(index, error);
                        continue;
                    }
                };
                self.parsed.insert(index, parsed);
                self.colors.insert(index, color(index));
            }
        }
    }

    fn bottom_panel(&mut self, ctx: &Context) {
        TopBottomPanel::bottom("bottom_panel").show(ctx, |ui| {
            bar(ui, |ui| {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    warn_if_debug_build(ui);
                    ui.spacing();
                    ui.label(RichText::new(env!("CARGO_PKG_VERSION")).small());
                });
            });
        });
    }

    fn central_panel(&mut self, ctx: &Context) {
        CentralPanel::default().show(ctx, |ui| {
            if self.files.is_empty() {
                ui.centered_and_justified(|ui| ui.label("Drag and drop .msp file"))
                    .response
            } else {
                ui.vertical_centered_justified(|ui| {
                    ui.heading(&self.parsed[&0].name);
                });
                ui.separator();
                self.plot(ui)
            }
        });
    }

    fn left_panel(&mut self, ctx: &Context) {
        SidePanel::left("left_panel").show_animated(ctx, self.left_panel, |ui| {
            ui.heading("Left Panel");
            ui.separator();
            ui.collapsing(WidgetText::from("Filter").heading(), |ui| {
                // Bounds
                ui.separator();
                ui.heading("Bounds");
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("Mass:");
                    let end = self.bounds.mass.end();
                    ui.drag_bound(&mut self.bounds.mass.0, |drag_value| {
                        drag_value.clamp_range(0..=end)
                    });
                    let start = self.bounds.mass.start();
                    ui.drag_bound(&mut self.bounds.mass.1, |drag_value| {
                        drag_value.clamp_range(start..=usize::MAX)
                    });
                });
                ui.horizontal(|ui| {
                    ui.label("Intensity:");
                    ui.drag_bound(&mut self.bounds.intensity, |drag_value| drag_value);
                });
                // Limits
                ui.separator();
                ui.heading("Limits");
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("Mass:");
                    ui.drag_option(
                        &mut self.limits.mass.0,
                        0..=self.limits.mass.1.unwrap_or(usize::MAX),
                        0.1,
                    );
                    ui.drag_option(
                        &mut self.limits.mass.1,
                        self.limits.mass.0.unwrap_or(0)..=usize::MAX,
                        0.1,
                    );
                });
                ui.horizontal(|ui| {
                    ui.label("Intensity:");
                    ui.drag_option(
                        &mut self.limits.intensity.0,
                        0..=self.limits.intensity.1.unwrap_or(u64::MAX),
                        0.1,
                    );
                    ui.drag_option(
                        &mut self.limits.intensity.1,
                        self.limits.intensity.0.unwrap_or(0)..=u64::MAX,
                        0.1,
                    );
                });
            });
            ui.collapsing(WidgetText::from("Finder").heading(), |ui| {
                // Input
                ui.separator();
                ui.heading("Input");
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("Mass:");
                    ui.add(DragValue::new(&mut self.mass).clamp_range(0..=self.bounds.mass.end()));
                    if ui.button("🔍").clicked() {}
                });
                let mut repeat = None;
                self.pattern.retain_mut(|step| {
                    ui.horizontal(|ui| {
                        if ui.button(RichText::new("-").monospace()).clicked() {
                            step.pop();
                            return !step.is_empty();
                        }
                        for variant in step.iter_mut() {
                            ui.add(DragValue::new(variant).clamp_range(0..=self.bounds.mass.end()));
                        }
                        if ui.button(RichText::new("+").monospace()).clicked() {
                            step.push(0);
                        }
                        if ui.button(RichText::new("🔃").monospace()).clicked() {
                            repeat = Some(step.clone());
                        }
                        true
                    })
                    .inner
                });
                if ui.button(RichText::new("+").monospace()).clicked() {
                    self.pattern.push(vec![0]);
                }
                if let Some(step) = repeat {
                    self.pattern.push(step);
                }
                // Output
                ui.separator();
                ui.heading("Output");
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("Count:");
                    ui.add(Slider::new(&mut self.count, 0..=10));
                });
            });
            ui.collapsing(WidgetText::from("Statistics").heading(), |ui| {
                ui.separator();
                ui.heading("Order");
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("Quantile:");
                    ui.drag_option(&mut self.statistics.quantile, 0.0..=1.0, 0.001);
                });
                ui.separator();
                ui.heading("Summary");
                ui.separator();
                ui.with_layout(Layout::top_down_justified(Align::LEFT), |ui| {
                    ui.group(|ui| {
                        ui.label("Mean:");
                        ui.checkbox(&mut self.statistics.mean, "Arithmetic")
                            .on_hover_text("Arithmetic mean");
                    });
                });
            });
            ui.collapsing(WidgetText::from("Visual").heading(), |ui| {
                ui.separator();
                ui.heading("Plot Legend");
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("Label:");
                    let mut selected = self.label.contains(Label::Index);
                    if ui.toggle_value(&mut selected, "Index").changed() {
                        self.label.set(Label::Index, selected);
                    }
                    selected = self.label.contains(Label::Mass);
                    if ui.toggle_value(&mut selected, "Mass").changed() {
                        self.label.set(Label::Mass, selected);
                    }
                    selected = self.label.contains(Label::Delta);
                    if ui.toggle_value(&mut selected, "Delta").changed() {
                        self.label.set(Label::Delta, selected);
                    }
                });
            });
            ui.collapsing(WidgetText::from("Trash").heading(), |ui| {
                // Peak detector
                ui.group(|ui| {
                    ui.heading("Peak detector");
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.label("Lag:");
                        ui.add(DragValue::new(&mut self.lag).clamp_range(1..=u64::MAX));
                    })
                    .response
                    .on_hover_text("the lag of the moving window");
                    ui.horizontal(|ui| {
                        ui.label("Threshold:");
                        ui.add(
                            DragValue::new(&mut self.threshold)
                                .clamp_range(0.0..=f64::MAX)
                                .speed(0.01),
                        );
                    })
                    .response
                    .on_hover_text("the z-score at which the algorithm signals");
                    ui.horizontal(|ui| {
                        ui.label("Influence:");
                        ui.add(
                            DragValue::new(&mut self.influence)
                                .clamp_range(0.0..=1.0)
                                .speed(0.01),
                        );
                    })
                    .response
                    .on_hover_text("the influence (between 0 and 1) of new signals");
                    // the influence (between 0 and 1) of new signals on the mean and standard deviation
                });
            });
            // ui.add(toggle(&mut self.temp));
        });
    }

    fn top_panel(&mut self, ctx: &Context, _frame: &mut Frame) {
        TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                global_dark_light_mode_switch(ui);
                ui.separator();
                ui.toggle_value(&mut self.left_panel, "🛠 Control");
                ui.toggle_value(&mut self.errors.show, "⚠ Errors");
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
                    self.errors.buffer.retain(|&index, error| {
                        ui.horizontal(|ui| {
                            ui.label(self.files[index].display().to_string())
                                .on_hover_text(error.to_string());
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
                    self.files.retain(with_index(|index, file: &DroppedFile| {
                        ui.horizontal(|ui| {
                            let mut include = !self.filter.contains(&index);
                            if ui.checkbox(&mut include, "").changed() {
                                if include {
                                    self.filter.remove(&index);
                                } else {
                                    self.filter.insert(index);
                                }
                            }
                            ui.label(file.display().to_string());
                            ui.color_edit_button_srgba(&mut self.colors[index]);
                            !ui.button("🗙").clicked()
                        })
                        .inner
                    }));
                });
            if !open {
                self.files.clear();
            }
        }
    }
}

impl App {
    fn plot(&self, ui: &mut Ui) -> Response {
        // let size = TextStyle::Body.resolve(ui.style()).size;
        let size = ui.text_style_height(&TextStyle::Body);
        let mut bar_charts = Vec::new();
        let mut lines = Vec::new();
        let mut points = Vec::new();
        let mut texts = Vec::new();

        let parsed = &self.parsed[&0];
        // Unfiltered bar chart
        let bars = parsed
            .peaks
            .iter()
            .map(|(&mass, &intensity)| Bar::new(mass as _, intensity as _).name(mass))
            .collect();
        bar_charts.push(
            BarChart::new(bars)
                .name("Unfiltered")
                .color(Color32::GRAY.linear_multiply(0.1)),
        );
        // Filtered bar chart
        let peaks = ui.memory_mut(|memory| {
            memory
                .caches
                .cache::<Bounded>()
                .get((&parsed.peaks, self.bounds))
        });
        let bars = peaks
            .iter()
            .map(|(&mass, &intensity)| Bar::new(mass as _, intensity as _).name(mass))
            .collect();
        bar_charts.push(
            BarChart::new(bars)
                .name("Filtered")
                .color(self.colors[0])
                .element_formatter(Box::new(
                    |Bar {
                         argument, value, ..
                     },
                     _| format!("{argument} {value}"),
                )),
        );
        // Statistics
        let mut intensities = Array1::from_iter(parsed.intensities());
        if self.statistics.mean {
            if let Some(mean) = intensities.mean() {
                lines.push(HLine::new(mean as f64).name("Mean").into());
            }
        }
        if let Some(quantile) = self.statistics.quantile {
            if let Ok(value) = intensities.quantile_mut(n64(quantile), &Linear) {
                lines.push(
                    HLine::new(value as f64)
                        .name(format_args!("Quantile {:.1}%", quantile * 100.0))
                        .into(),
                );
            }
        }
        // Find
        // let p = vec![vec![15], vec![12, 12, 14, 14, 14, 14, 14, 14]];
        // let pattern = &[
        //     vec![14],
        //     vec![12, 14],
        //     vec![12, 14],
        // ];
        // [[0, 0, 0]] = 1
        // [[0, 0, 1]] = 2
        // [[0, 1, 0]] = 3
        // [[0, 1, 1]] = 4
        //    [
        //       [
        //         [14, 12],
        //         [14, 12]
        //       ],
        //       [
        //         [14, 14],
        //         [14, 14]
        //       ]
        //     ]
        let predictions = ui.memory_mut(|memory| {
            memory.caches.cache::<Predicted>().get(Key {
                mass: self.mass,
                peaks: &peaks,
                pattern: &self.pattern,
                zero_is_included: (self.bounds.intensity, Bound::Unbounded).contains(&0),
            })
        });
        for (i, prediction) in predictions.into_iter().take(self.count).enumerate().rev() {
            let color = color(i);
            let mut series = Vec::with_capacity(prediction.0.ndim());
            let mut mass = self.mass;
            for j in 0..prediction.0.ndim() {
                let delta = self.pattern[j][prediction.0[j]];
                mass -= delta;
                let intensity = peaks.get(&mass).copied().unwrap_or_default();
                series.push([mass as f64, intensity as f64]);
                let mut text = String::new();
                if self.label.contains(Label::Index) {
                    writeln!(text, "{j}").ok();
                }
                if self.label.contains(Label::Mass) {
                    writeln!(text, "{mass}").ok();
                }
                if self.label.contains(Label::Delta) {
                    writeln!(text, "{delta}").ok();
                }
                // let mut job = LayoutJob::default();
                // job.append(&text, 5.0 * size as f32, default());
                // // let mut job = LayoutJob::simple(text, default(), color(i), 0.0);
                // job.halign = Align::Center;
                texts.push(
                    Text::new(
                        PlotPoint::new(mass as f64, intensity as f64),
                        RichText::new(text).monospace().size(size),
                    )
                    .anchor(Align2::CENTER_BOTTOM)
                    .color(color)
                    .name(format_args!("Prediction {i}")),
                );
            }
            points.push(
                Points::new(series)
                    .color(color)
                    .filled(true)
                    .radius(size / 2.0)
                    .shape(MarkerShape::Circle)
                    .name(format_args!("Prediction {i}")),
            );
        }
        // Limits
        if let Some(value) = self.limits.mass.0 {
            lines.push(VLine::new(value as f64).name("Min mass").into());
        }
        if let Some(value) = self.limits.mass.1 {
            lines.push(VLine::new(value as f64).name("Max mass").into());
        }
        if let Some(value) = self.limits.intensity.0 {
            lines.push(HLine::new(value as f64).name("Min intensity").into());
        }
        if let Some(value) = self.limits.intensity.1 {
            lines.push(HLine::new(value as f64).name("Max intensity").into());
        }
        // Plot
        Plot::new("plot")
            .legend(Legend::default())
            .coordinates_formatter(Corner::LeftBottom, CoordinatesFormatter::default())
            .show(ui, |plot_ui| {
                for bar_chart in bar_charts {
                    plot_ui.bar_chart(bar_chart);
                }
                for line in lines {
                    match line {
                        Line::Horizontal(line) => plot_ui.hline(line),
                        Line::Vertical(line) => plot_ui.vline(line),
                        Line::Diagonal(line) => plot_ui.line(line),
                    }
                }
                for points in points {
                    plot_ui.points(points);
                }
                for text in texts {
                    plot_ui.text(text);
                }
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

/// Line
enum Line {
    Horizontal(HLine),
    Vertical(VLine),
    Diagonal(plot::Line),
}

impl From<HLine> for Line {
    fn from(value: HLine) -> Self {
        Self::Horizontal(value)
    }
}

impl From<VLine> for Line {
    fn from(value: VLine) -> Self {
        Self::Vertical(value)
    }
}

impl From<plot::Line> for Line {
    fn from(value: plot::Line) -> Self {
        Self::Diagonal(value)
    }
}

/// Bounds
#[derive(Clone, Copy, Debug, Deserialize, Hash, Serialize)]
struct Bounds {
    mass: (Bound<usize>, Bound<usize>),
    intensity: Bound<u64>,
}

impl Default for Bounds {
    fn default() -> Self {
        Self {
            mass: (Bound::Unbounded, Bound::Unbounded),
            intensity: Bound::Unbounded,
        }
    }
}

/// Errors
#[derive(Debug, Default)]
struct Errors {
    show: bool,
    buffer: IndexMap<usize, Error>,
}

bitflags! {
    /// Label
    #[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
    struct Label: u8 {
        const Index = 0b001;
        const Mass = 0b010;
        const Delta = 0b100;
    }
}

impl fmt::Display for Label {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

/// Limits
#[derive(Clone, Copy, Debug, Default, Deserialize, Hash, Serialize)]
struct Limits {
    mass: (Option<usize>, Option<usize>),
    intensity: (Option<u64>, Option<u64>),
}

/// Statistics
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
struct Statistics {
    mean: bool,
    quantile: Option<f64>,
}

mod bounder;
mod predictioner;

#[cfg(test)]
mod test {
    use super::*;
    use itertools::Itertools;
    use ndarray::{arr0, arr1, aview0, aview1, aview2, ArrayD, Axis, Dimension};
    use petgraph::{
        algo::{astar, bellman_ford, dijkstra, is_isomorphic_subgraph_matching},
        prelude::UnGraph,
        Direction, Graph, Undirected,
    };
    use std::iter::zip;

    // #[test]
    // fn test0() {
    //     let permutations = [12, 12, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14]
    //         .into_iter()
    //         .permutations(12)
    //         .unique();
    //     println!("permutations: {}", permutations.count());
    // }

    #[test]
    fn test0() {
        let mut a = ArrayD::<f64>::default(&[0][..]);
        println!("a: {a}, {:?}", a.shape());
        a.insert_axis_inplace(Axis(0));
        println!("a: {a}, {:?}", a.shape());
        a.push(Axis(1), aview1(&[15.0]).into_dyn()).unwrap();
        println!("a: {a}, {:?}", a.shape());
    }

    #[test]
    fn test2() {
        // Поиск оптимального пути
        // Совпадение шаблона с вилд символами
        let intensities = (0..256).collect_vec();
        let pattern = &[
            vec![15], //
            vec![12, 14],
            vec![12, 14],
            vec![12, 14],
        ];
        let mass = 255usize;
        let shape = pattern.iter().map(Vec::len).collect::<Vec<_>>();
        let a = ArrayD::from_shape_fn(shape, |dimension| {
            println!("dimension: {:?}", dimension.slice());
            let mut m = mass;
            let mut intensity = 0;
            for dm in zip(pattern, dimension.slice()).map(|(step, &index)| step[index]) {
                m -= dm;
                intensity += intensities[m];
                println!("m: {m}, dm: {dm}, intensity: {}", intensities[m]);
            }
            println!("intensity: {intensity}");
            intensity as f64
        });

        // let mut a = ArrayD::<usize>::zeros(shape);
        // for index in 0..pattern.len() {
        //     for (i, mut array) in a.axis_iter_mut(Axis(index)).enumerate() {
        //         array += int[mass - pattern[index][i]];
        //         // println!("i: {i}");
        //         // mass = mass - pattern[index][i];
        //         // println!("mass: {mass}");
        //     }
        // }

        // let mut a = ArrayD::<usize>::zeros(IxDyn(&ix));
        // for (i, mut array) in a.axis_iter_mut(Axis(0)).enumerate() {
        //     let mass = mass - pattern[0][i];
        //     println!("i {mass}");
        //     for (j, mut array) in array.axis_iter_mut(Axis(0)).enumerate() {
        //         let mass = mass - pattern[1][j];
        //         println!("j {mass}");
        //         for (k, mut array) in array.axis_iter_mut(Axis(0)).enumerate() {
        //             let mass = mass - pattern[2][k];
        //             println!("k {mass}");
        //             for (l, mut array) in array.axis_iter_mut(Axis(0)).enumerate() {
        //                 let mass = mass - pattern[3][l];
        //                 println!("{i}, {j}, {k}, {l}, {array}");
        //                 array[[]] = mass;
        //             }
        //         }
        //     }
        // }
        println!("{a}");
        // for (dimensions, array) in array.indexed_iter_mut() {
        //     let mut mass = mass;
        //     for i in 0..pattern.len() {
        //         mass -= pattern[i][dimensions[i]];
        //         // println!("{mass}: {:?}", int[mass]);
        //     }
        //     // let mass = mass - pattern[1][dimensions[1]];
        //     // println!("{mass}");
        //     // for &index in &ix {
        //     // }
        // }
    }

    #[test]
    fn test3() {
        let pattern = &[
            vec![15], //
            vec![12, 14],
            vec![12, 14],
            vec![12, 14],
        ];
        // let shape = pattern.iter().map(Vec::len).collect::<Vec<_>>();
        // let mut a = Array2::from(vec![0usize]).into_dyn();
        // println!("a: {a}, {:?}, {:?}", a.dim(), a.shape());
        // a.push(Axis(0), aview0(&9).into_dyn()).unwrap();
        // a.insert_axis_inplace(Axis(0));
        // for i in a {
        //     println!("i: {i}");
        // }

        // let mut a = ArrayD::<f64>::zeros(shape);
        // println!("a: {a}, {:?}, {:?}, {:?}", a.raw_dim(), a.dim(), a.shape());
        // let mut b = ArrayD::<f64>::zeros(a.shape());
        // println!("b: {b}, {:?}, {:?}, {:?}", b.raw_dim(), b.dim(), b.shape());
        // // [1]; -> [1, 1]; -> [1, 2];
        // // let c = ArrayD::<usize>::from_shape_fn(&[][..], |dimension| {
        // let mut c = ArrayD::<usize>::default(IxDyn::default());
        // println!("c: {c}, {:?}, {:?}, {:?}", c.raw_dim(), c.dim(), c.shape());
        // c.push(Axis(0), aview0(&9).into_dyn()).unwrap();
        // println!("c: {c}, {:?}, {:?}, {:?}", c.raw_dim(), c.dim(), c.shape());
        // c.insert_axis_inplace(Axis(0));
        // println!("c: {c}, {:?}, {:?}, {:?}", c.raw_dim(), c.dim(), c.shape());
        // c.push(Axis(1), aview1(&[8]).into_dyn()).unwrap();
        // println!("c: {c}, {:?}, {:?}, {:?}", c.raw_dim(), c.dim(), c.shape());
    }

    #[test]
    fn test1() {
        // let methane = {
        //     let molecule = Molecule::new_undirected();
        //     molecule
        // };
        let g0 = {
            let mut g = Graph::new_undirected();
            // C
            let a = g.add_node(12);
            let b = g.add_node(12);
            g.add_edge(a, b, 1);
            println!("g: {g:?}");
            g
        };
        let g1 = {
            let mut g = Graph::new_undirected();
            // C
            let a = g.add_node(12);
            let b = g.add_node(12);
            let c = g.add_node(12);
            g.add_edge(a, b, 1);
            g.add_edge(b, c, 1);
            println!("g: {g:?}");
            g
        };
        let g2 = {
            let mut g = Graph::new_undirected();
            // C
            let c0 = g.add_node(12);
            let c1 = g.add_node(12);
            let c2 = g.add_node(12);
            // H
            let h0 = g.add_node(1);
            let h1 = g.add_node(1);
            let h2 = g.add_node(1);
            let h3 = g.add_node(1);
            let h4 = g.add_node(1);
            let h5 = g.add_node(1);
            let h6 = g.add_node(1);
            let h7 = g.add_node(1);
            g.extend_with_edges(&[
                (c0, c1, 1),
                (c1, c2, 1),
                (c0, h0, 1),
                (c0, h1, 1),
                (c0, h2, 1),
                (c1, h3, 1),
                (c1, h4, 1),
                (c2, h5, 1),
                (c2, h6, 1),
                (c2, h7, 1),
            ]);
            println!("g: {g:?}");
            g
        };
        let g3 = {
            let mut g = Graph::new_undirected();
            // C
            let c0 = g.add_node(12);
            let c1 = g.add_node(12);
            let c2 = g.add_node(12);
            // H
            let h0 = g.add_node(1);
            let h1 = g.add_node(1);
            let h2 = g.add_node(1);
            let h3 = g.add_node(1);
            let h4 = g.add_node(1);
            let h5 = g.add_node(1);
            g.extend_with_edges(&[
                (c0, c1, 1),
                (c1, c2, 2),
                (c0, h0, 1),
                (c0, h1, 1),
                (c0, h2, 1),
                (c1, h3, 1),
                (c2, h4, 1),
                (c2, h5, 1),
            ]);
            println!("g: {g:?}");
            g
        };
        let g4 = {
            let mut g = Graph::new_undirected();
            // C
            let c0 = g.add_node(12);
            let c1 = g.add_node(12);
            let c2 = g.add_node(12);
            // H
            let h0 = g.add_node(1);
            let h1 = g.add_node(1);
            let h2 = g.add_node(1);
            let h3 = g.add_node(1);
            let h4 = g.add_node(1);
            let h5 = g.add_node(1);
            g.extend_with_edges(&[
                (c0, c1, 2),
                (c1, c2, 1),
                (c0, h0, 1),
                (c0, h1, 1),
                (c1, h2, 1),
                (c2, h3, 1),
                (c2, h4, 1),
                (c2, h5, 1),
            ]);
            println!("g: {g:?}");
            g
        };
        let check = is_isomorphic_subgraph_matching(&g0, &g1, |x, y| x == y, |x, y| x == y);
        println!("check: {check}");
        let check = is_isomorphic_subgraph_matching(&g3, &g4, PartialEq::eq, PartialEq::eq);
        println!("check: {check}");

        // for node in g.neighbors_directed(a, Direction::Outgoing) {
        //     println!("i: {node:?}, {:?}", g.node_weight(node));
        // }
        // for edge in g.edges(a) {
        //     println!("edge: {edge:?}, {:?}", edge.weight());
        // }
        // let astar_map = astar(&g, start, |node| node == end, |edge| *edge.weight(), |_| 0);
        // println!("astar: {:?}", astar_map);

        // Z is disconnected.
        // let _ = g.add_node("Z");
        // g.add_edge(a, aa, 12);
        // g.add_edge(a, ab, 14);
        // g.add_edge(b, ba, 12);
        // g.add_edge(b, bb, 14);

        // g.add_edge(h, j, 3.);
        // g.add_edge(i, j, 1.);
        // g.add_edge(i, k, 2.);
    }
}
