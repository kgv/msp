use super::{InnerResponseExt, ResponseExt};
use crate::utils::BoundExt;
use eframe::emath::Numeric;
use egui::{DragValue, Response, Ui, Widget};
use serde::{Deserialize, Serialize};
use std::ops::Bound;

/// Extension methods for [`Ui`]
pub trait UiExt {
    fn drag_bound<T>(
        &mut self,
        bound: &mut Bound<T>,
        f: impl FnMut(DragValue) -> DragValue,
    ) -> Response
    where
        for<'a> T: Numeric + Serialize + Deserialize<'a> + Send + Sync;

    fn drag_percent<T: Numeric>(&mut self, value: &mut T) -> Response;
}

impl UiExt for Ui {
    fn drag_bound<T>(
        &mut self,
        bound: &mut Bound<T>,
        mut f: impl FnMut(DragValue) -> DragValue,
    ) -> Response
    where
        for<'a> T: Numeric + Serialize + Deserialize<'a> + Send + Sync,
    {
        let id = self.id().with("value");
        let mut value = bound.value().copied().unwrap_or_else(|| {
            self.data_mut(|data| data.get_persisted(id).unwrap_or(T::from_f64(f64::INFINITY)))
        });
        match bound {
            Bound::Unbounded => self
                .add_enabled_ui(false, |ui| ui.add(f(DragValue::new(&mut value))))
                .flatten(),
            Bound::Included(value) | Bound::Excluded(value) => self.add(f(DragValue::new(value))),
        }
        .on_hover_text(bound.variant_name())
        .context_menu(|ui| {
            let response = ui.selectable_value(bound, Bound::Included(value), "Included")
                | ui.selectable_value(bound, Bound::Excluded(value), "Excluded")
                | ui.selectable_value(bound, Bound::Unbounded, "Unbounded")
                    .with_clicked(|| {
                        self.data_mut(|data| data.insert_persisted(id, value));
                    });
            if response.clicked() {
                ui.close_menu();
            }
            // if ui.ui_contains_pointer() && ui.input(|input| input.pointer.any_click()) {
            //     ui.close_menu();
            // }
        })
    }

    fn drag_percent<T: Numeric>(&mut self, value: &mut T) -> Response {
        DragValue::new(value)
            .clamp_range(0..=100)
            .speed(0.1)
            .suffix('%')
            .ui(self)
    }
}
