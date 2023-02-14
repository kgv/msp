use std::{fmt::Display, ops::BitOr};

use eframe::emath::Numeric;
use egui::{collapsing_header::CollapsingState, DragValue, Response, Sense, Ui, Vec2, Widget};

/// Extension methods for [`CollapsingState`]
pub(crate) trait CollapsingStateExt {
    fn open(self, open: Option<bool>) -> Self;
}

impl CollapsingStateExt for CollapsingState {
    fn open(mut self, open: Option<bool>) -> Self {
        if let Some(open) = open {
            self.set_open(open);
        }
        self
    }
}

/// Extension methods for [`Ui`]
pub(crate) trait UiExt {
    fn drag_percent<T: Numeric>(&mut self, value: &mut T) -> Response;
}

impl UiExt for Ui {
    fn drag_percent<T: Numeric>(&mut self, value: &mut T) -> Response {
        DragValue::new(value)
            .clamp_range(0..=100)
            .speed(0.1)
            .suffix('%')
            .ui(self)
    }
}

// pub trait OptionalWidget<T> {
//     fn optional_widget<F: FnMut() -> Widget>(&mut self, f: F) -> Response;
// }

pub trait SelectableValueFromIter<T> {
    fn selectable_value_from_iter(
        &mut self,
        current_value: &mut T,
        values: impl Iterator<Item = T>,
    ) -> Response;
}

impl<T> SelectableValueFromIter<T> for Ui
where
    T: PartialEq + Display + Copy,
{
    fn selectable_value_from_iter(
        &mut self,
        current_value: &mut T,
        values: impl Iterator<Item = T>,
    ) -> Response {
        values
            .map(|value| self.selectable_value(current_value, value, format!("{value}")))
            .reduce(BitOr::bitor)
            .unwrap_or_else(|| {
                self.colored_label(self.style().visuals.error_fg_color, "⚠ No items")
            })
    }
}
