use self::{
    bounder::{Bounded, Bounder},
    predictioner::{Predicted, Predictioner},
};
use crate::{
    parser::Parsed,
    utils::{
        stats::Summary, with_index, BoundExt, Display, DroppedFileExt, InnerResponseExt, Stats,
        UiExt,
    },
};
use anyhow::{bail, Context as _, Error, Result};
use eframe::{epaint::Hsva, get_value, set_value, CreationContext, Frame, Storage, APP_KEY};
use egui::{
    global_dark_light_mode_switch,
    menu::{self, bar},
    plot::{
        self, Bar, BarChart, BoxElem, BoxPlot, BoxSpread, CoordinatesFormatter, Corner, HLine,
        Legend, MarkerShape, Plot, PlotPoint, Points, Text, VLine,
    },
    popup_below_widget,
    text::LayoutJob,
    util::cache::{ComputerMut, FrameCache},
    warn_if_debug_build, Align, Align2, CentralPanel, Color32, ComboBox, Context, DragValue,
    DroppedFile, FontId, Id, LayerId, Layout, Order, Response, RichText, SidePanel, Style,
    TextStyle, TopBottomPanel, Ui, Visuals, WidgetText, Window,
};
use indexmap::IndexMap;
use itertools::Itertools;
use ndarray::{array, Array, Array1, Array2, ArrayD, Axis, Dimension, IxDyn};
use ndarray_stats::{
    interpolate::{Linear, Midpoint},
    Quantile1dExt, QuantileExt, SummaryStatisticsExt,
};
use noisy_float::types::{n64, N64};
use petgraph::Graph;
use serde::{Deserialize, Serialize};
use smoothed_z_score::{PeaksDetector, PeaksFilter};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    convert::identity,
    default::default,
    fmt::Write,
    fs::read_to_string,
    iter::zip,
    ops::{Bound, Deref, RangeBounds},
};
use tracing::{error, info, trace};

const MARKER_SHAPES: [MarkerShape; 10] = [
    MarkerShape::Circle,
    MarkerShape::Diamond,
    MarkerShape::Square,
    MarkerShape::Cross,
    MarkerShape::Plus,
    MarkerShape::Up,
    MarkerShape::Down,
    MarkerShape::Left,
    MarkerShape::Right,
    MarkerShape::Asterisk,
];

fn color(index: usize) -> Color32 {
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
    zero: bool,
    pattern: Vec<Vec<usize>>,

    // Statistics
    statistics: Statistics,

    // Peak Finder
    lag: usize,
    threshold: f64,
    influence: f64,

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
                ui.toggle_value(&mut self.errors.show, "⚠");
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
        SidePanel::left("left_panel").show(ctx, |ui| {
            ui.heading("Left Panel");
            ui.separator();
            ui.collapsing(WidgetText::from("Filter").heading(), |ui| {
                // Bounds
                ui.heading("Bounds");
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("Mass:");
                    ui.drag_bound(&mut self.bounds.mass.0, |drag_value| {
                        let end = self.bounds.mass.1.value().copied().unwrap_or(usize::MAX);
                        drag_value.clamp_range(0..=end)
                    });
                    ui.drag_bound(&mut self.bounds.mass.1, |drag_value| {
                        let start = self.bounds.mass.0.value().copied().unwrap_or(0);
                        drag_value.clamp_range(start..=usize::MAX)
                    });
                });
                ui.horizontal(|ui| {
                    ui.label("Intensity:");
                    ui.drag_bound(&mut self.bounds.intensity, |drag_value| drag_value);
                });
                // Limits
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
                ui.heading("Input");
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("Mass:");
                    ui.add(DragValue::new(&mut self.mass).clamp_range(
                        0..=self.bounds.mass.1.value().copied().unwrap_or(usize::MAX),
                    ));
                    if ui.button("🔍").clicked() {}
                });
                ui.with_layout(Layout::top_down_justified(Align::LEFT), |ui| {
                    ui.group(|ui| {
                        ui.label("Intensity:");
                        ui.checkbox(&mut self.zero, "Zero")
                            .on_hover_text("Allows masses with zero intensity");
                    });
                });
                self.pattern.retain_mut(|step| {
                    ui.horizontal(|ui| {
                        if ui.button(RichText::new("-").monospace()).clicked() {
                            if step.pop().is_none() {
                                return false;
                            }
                        }
                        for variant in step.iter_mut() {
                            ui.add(DragValue::new(variant).clamp_range(
                                0..=self.bounds.mass.1.value().copied().unwrap_or(usize::MAX),
                            ));
                        }
                        if ui.button(RichText::new("+").monospace()).clicked() {
                            step.push(0);
                        }
                        true
                    })
                    .inner
                });
                if ui.button(RichText::new("+").monospace()).clicked() {
                    self.pattern.push(Vec::new());
                }
                // Output
                ui.heading("Output");
                ui.separator();
            });
            ui.collapsing(WidgetText::from("Statistics").heading(), |ui| {
                ui.heading("Order");
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("Quantile:");
                    ui.drag_option(&mut self.statistics.quantile, 0.0..=1.0, 0.001);
                });
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
                ui.heading("Plot Legend");
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("Label:");
                    ui.selectable_value(&mut self.label, Label::Mass, "Mass");
                    ui.selectable_value(&mut self.label, Label::Delta, "Delta");
                    ui.selectable_value(&mut self.label, Label::Index, "Index");
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
        });
    }

    fn top_panel(&mut self, ctx: &Context, _frame: &mut Frame) {
        TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                global_dark_light_mode_switch(ui);
                ui.separator();
                ui.toggle_value(&mut self.left_panel, "🛠 Control");
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
        let unfiltered = parsed.intensities();
        // Unfiltered bar chart
        let bars = unfiltered
            .iter()
            .enumerate()
            .map(|(mass, &intensity)| Bar::new(mass as _, intensity as _).name(mass))
            .collect();
        bar_charts.push(
            BarChart::new(bars)
                .name("Unfiltered")
                .color(Color32::GRAY.linear_multiply(0.1)),
        );
        // let array = Array1::from_iter(unfiltered.iter().map(|&i| i as f64));
        let mut array = Array1::from_iter(unfiltered.clone());
        if self.statistics.mean {
            if let Some(mean) = array.mean() {
                lines.push(HLine::new(mean as f64).name("Mean").into());
            }
        }
        if let Some(quantile) = self.statistics.quantile {
            if let Ok(value) = array.quantile_mut(n64(quantile), &Linear) {
                lines.push(
                    HLine::new(value as f64)
                        .name(format_args!("Quantile {:.1}%", quantile * 100.0))
                        .into(),
                );
            }
        }
        // Filtered bar chart
        let filtered = ui.memory_mut(|memory| {
            memory
                .caches
                .cache::<Bounded>()
                .get((&unfiltered, self.bounds))
        });
        let bars = filtered
            .iter()
            .enumerate()
            .filter_map(|(mass, &intensity)| {
                (intensity != 0).then_some(Bar::new(mass as _, intensity as _).name(mass))
            })
            .collect();
        bar_charts.push(
            BarChart::new(bars)
                .name("Filtered")
                .color(self.colors[0])
                .element_formatter(Box::new(|bar, _| format!("{} {}", bar.argument, bar.value))),
        );
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
        let shape = self.pattern.iter().map(Vec::len).collect::<Vec<_>>();
        let predictions = ArrayD::from_shape_fn(shape, |dimension| {
            let mut mass = self.mass;
            let mut intensity = 0.0;
            for delta in zip(&self.pattern, dimension.slice()).map(|(step, &index)| step[index]) {
                mass = match mass.checked_sub(delta) {
                    None => return 0.0,
                    Some(mass) => mass,
                };
                intensity += match filtered[mass] {
                    0 if !self.zero => return 0.0,
                    intensity => intensity as f64,
                };
            }
            intensity
        });
        error!(?predictions);
        if let Ok(prediction) = predictions.argmax() {
            error!(?prediction);
            let mut mass = self.mass;
            let mut series = Vec::with_capacity(prediction.ndim());
            let mut push = |index: usize, delta: usize, mass: usize, intensity: u64| {
                let text = match self.label {
                    Label::Mass => mass.to_string(),
                    Label::Delta => delta.to_string(),
                    Label::Index => index.to_string(),
                };
                series.push([mass as f64, intensity as f64]);
                texts.push(
                    Text::new(
                        PlotPoint::new(mass as f64, 2.0 * size as f64 + intensity as f64),
                        RichText::new(text).size(size),
                    )
                    .name("Labels")
                    .highlight(true),
                );
            };
            push(0, 0, mass, filtered[mass]);
            for index in 0..=prediction.ndim() {
                let delta = self.pattern[index][prediction[index]];
                mass -= delta;
                let intensity = filtered[mass];
                push(index + 1, delta, mass, intensity);
            }
            points.push(
                Points::new(series)
                    .color(color(9))
                    .filled(true)
                    .radius(size / 2.0)
                    .shape(MarkerShape::Circle)
                    .name("Points"),
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

        // let permutations = [12, 12, 14, 14, 14, 14, 14, 14]
        //     .into_iter()
        //     .permutations(8)
        //     .unique();
        // let points = permutations
        //     .filter_map(|permutation| {
        //         // error!(len = %filtered.len(), ?filtered);
        //         let prediction = ui.memory_mut(|memory| {
        //             memory
        //                 .caches
        //                 .cache::<Predicted>()
        //                 .get((&permutation, &filtered[0..=self.mass]))
        //         });
        //         if prediction < f64::EPSILON {
        //             return None;
        //         }
        //         // error!(?permutation);
        //         let mut mass = self.mass;
        //         let mut series = vec![[mass as _, filtered[mass] as _]];
        //         for &delta in &permutation {
        //             mass -= delta;
        //             let intensity = filtered[mass];
        //             series.push([mass as _, intensity as _]);
        //         }
        //         Some(
        //             Points::new(series)
        //                 .filled(true)
        //                 .radius(9.0)
        //                 .shape(MarkerShape::Circle)
        //                 .name(prediction),
        //         )
        //     })
        //     .collect::<Vec<_>>();

        // let points = filtered
        //     .iter()
        //     .enumerate()
        //     .rev()
        //     .map(|(mass, &intensity)| {
        //         permutations
        //             .clone()
        //             .filter_map(|permutation| {
        //                 let permutations = ui.memory_mut(|memory| {
        //                     let cache = memory.caches.cache::<Predicted>();
        //                     cache.get((&permutation, &filtered))
        //                 });
        //                 // error!(?permutation);
        //                 let mut mass = mass;
        //                 let mut accumulator = 1.0;
        //                 let mut series = vec![[mass as f64, intensity as _]];
        //                 for &dm in &permutation {
        //                     mass = mass.checked_sub(dm)?;
        //                     let intensity = filtered[mass];
        //                     if intensity < 25.0 {
        //                         return None;
        //                     }
        //                     accumulator *= intensity / sum;
        //                     series.push([mass as _, intensity as _]);
        //                 }
        //                 Some(
        //                     Points::new(series)
        //                         .filled(true)
        //                         .radius(9.0)
        //                         .shape(MarkerShape::Circle)
        //                         .name(accumulator),
        //                 )
        //             })
        //             .collect()
        //     })
        //     .collect::<Vec<Vec<_>>>();
        // error!(?points);
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

/// Mass
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
enum Label {
    #[default]
    Mass,
    Delta,
    Index,
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

// mod permutationer;
mod bounder;
mod predictioner;

#[cfg(test)]
mod test {
    use super::*;
    use ndarray::{arr0, arr1, aview0, aview1, aview2, Dimension};
    use petgraph::{
        algo::{astar, bellman_ford, dijkstra},
        Direction, Graph,
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
        let shape = pattern.iter().map(Vec::len).collect::<Vec<_>>();
        let mut a = ArrayD::<f64>::zeros(shape);
        println!("a: {a}, {:?}, {:?}, {:?}", a.raw_dim(), a.dim(), a.shape());
        let mut b = ArrayD::<f64>::zeros(a.shape());
        println!("b: {b}, {:?}, {:?}, {:?}", b.raw_dim(), b.dim(), b.shape());
        // [1]; -> [1, 1]; -> [1, 2];
        // let c = ArrayD::<usize>::from_shape_fn(&[][..], |dimension| {
        let mut c = ArrayD::<usize>::default(IxDyn::default());
        println!("c: {c}, {:?}, {:?}, {:?}", c.raw_dim(), c.dim(), c.shape());
        c.push(Axis(0), aview0(&9).into_dyn()).unwrap();
        println!("c: {c}, {:?}, {:?}, {:?}", c.raw_dim(), c.dim(), c.shape());
        c.insert_axis_inplace(Axis(0));
        println!("c: {c}, {:?}, {:?}, {:?}", c.raw_dim(), c.dim(), c.shape());
        c.push(Axis(1), aview1(&[8]).into_dyn()).unwrap();
        println!("c: {c}, {:?}, {:?}, {:?}", c.raw_dim(), c.dim(), c.shape());

        // c.push(Axis(1), aview1(&[7]).into_dyn()).unwrap();
        // println!("c: {c}, {:?}, {:?}, {:?}", c.raw_dim(), c.dim(), c.shape());
        // c.push(Axis(0), aview1(&[6, 5, 4]).into_dyn()).unwrap();
        // println!("c: {c}, {:?}, {:?}, {:?}", c.raw_dim(), c.dim(), c.shape());

        // let d = ArrayD::<usize>::default(&[0][..]);
        // println!("d: {d}, {:?}, {:?}, {:?}", d.raw_dim(), d.dim(), d.shape());

        // t.append(Axis(0), aview1(&[15]).into_dyn()).unwrap();
        // println!("1: {t}");
        // t.append(Axis(1), aview1(&[12, 14]).into_dyn()).unwrap();
        // println!("2: {t}");
        // let mut u = t.insert_axis(Axis(0));
    }

    #[test]
    fn test1() {
        use std::hash::Hash;
        fn h<H: Hash>(h: H) {}

        let mut g = Graph::new();
        // h(g);
        let start = g.add_node(0);
        let a = g.add_node(1);
        g.add_edge(start, a, 0);
        let aa = g.add_node(1);
        let ab = g.add_node(1);
        g.add_edge(a, aa, 12);
        g.add_edge(a, ab, 14);
        let aaa = g.add_node(1);
        let abb = g.add_node(1);
        g.add_edge(aa, aaa, 12);
        g.add_edge(ab, aaa, 12);
        g.add_edge(aa, abb, 14);
        g.add_edge(ab, abb, 14);
        let end = g.add_node(0);
        g.add_edge(aaa, end, 0);
        g.add_edge(abb, end, 0);

        println!("g: {g:?}");
        for node in g.neighbors_directed(a, Direction::Outgoing) {
            println!("i: {node:?}, {:?}", g.node_weight(node));
        }
        for edge in g.edges(a) {
            println!("edge: {edge:?}, {:?}", edge.weight());
        }
        let astar_map = astar(&g, start, |node| node == end, |edge| *edge.weight(), |_| 0);
        println!("astar: {:?}", astar_map);

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
