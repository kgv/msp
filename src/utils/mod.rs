pub(crate) use self::{
    bound::BoundExt,
    display::Trait as Display,
    egui::{CollapsingStateExt, UiExt},
    float::FloatExt,
    stats::Stats,
};

mod bound;
mod display;
mod egui;
mod float;
mod stats;
mod string;
