//! [graphonline.ru](https://graphonline.ru/)

use egui::{lerp, pos2, vec2, Response, Sense, Ui, Widget, WidgetInfo, WidgetType};
use num_traits::{FromPrimitive, Num, NumCast};
use petgraph::{
    algo::is_isomorphic_subgraph_matching,
    graph::{DefaultIx, EdgeIndex, NodeIndex},
    prelude::Graph,
    Undirected,
};

use crate::app::color;

pub fn toggle(value: &mut f64) -> impl Widget + '_ {
    move |ui: &mut Ui| toggle_ui(ui, value)
}

fn toggle_ui(ui: &mut Ui, value: &mut f64) -> Response {
    let desired_size = ui.spacing().interact_size.y * vec2(12.0, 12.0);
    let (rect, mut response) = ui.allocate_exact_size(desired_size, Sense::click_and_drag());
    if response.clicked() {
        // *on = !*on;
        response.mark_changed();
    }
    response.widget_info(|| WidgetInfo::drag_value(*value));
    if ui.is_rect_visible(rect) {
        // let how_on = ui.ctx().animate_bool(response.id, *on);
        let visuals = ui.style().interact(&response);
        let rect = rect.expand(visuals.expansion);
        ui.painter().circle(
            rect.center(),
            0.5 * rect.height(),
            visuals.bg_fill,
            visuals.fg_stroke,
        );

        // let radius = 0.5 * rect.height();
        // ui.painter()
        //     .rect(rect, radius, visuals.bg_fill, visuals.bg_stroke);
        // let circle_x = lerp((rect.left() + radius)..=(rect.right() - radius), how_on);
        // let center = pos2(circle_x, rect.center().y);
        // ui.painter()
        //     .circle(center, 0.75 * radius, visuals.bg_fill, visuals.fg_stroke);
    }
    response
}

pub mod atom;
pub mod molecule;
