use std::rc::Rc;

use rikka_app::prelude::*;
use rikka_widgets::column;

use crate::performance::graph::Graph;

pub(crate) struct Plot {
    pub(crate) samples: Rc<Vec<f32>>,
    pub(crate) second: Option<Rc<Vec<f32>>>,
    pub(crate) scale: f32,
    pub(crate) colour: Color,
    pub(crate) ceiling: String,
    pub(crate) caption: Label,
}

pub(crate) fn header(title: String, detail: String) -> impl View {
    row! {
        text(title).display();
        spacer();
        text(detail).label().muted().no_wrap();
    }
    .gap(Space::ROOMY)
    .align_items(Align::End)
}

pub(crate) fn plot(plot: Plot, grow: f32) -> impl View {
    let muted = palette().text_muted;
    column! {
        row! {
            text(plot.caption).caption().color(muted);
            spacer();
            text(plot.ceiling).caption().color(muted);
        };
        Element::new(Graph {
            samples: plot.samples,
            second: plot.second,
            scale: plot.scale,
            colour: plot.colour,
            ruled: true,
        })
        .fill_width()
        .grow(grow);
        row! {
            text(tr!("tasks.graph.span")).caption().color(muted);
            spacer();
            text(tr!("tasks.graph.zero")).caption().color(muted);
        };
    }
    .gap(Space::TIGHT)
    .grow(grow)
}

pub(crate) fn footer(stats: impl View, details: impl View) -> impl View {
    row! {
        stats;
        spacer();
        details;
    }
    .gap(Space::LOOSE)
}

pub(crate) fn percentage(samples: Rc<Vec<f32>>, colour: Color, caption: Label) -> impl View {
    plot(
        Plot {
            samples,
            second: None,
            scale: 1.0,
            colour,
            ceiling: crate::format::percent(1.0, 0),
            caption,
        },
        1.0,
    )
}

pub(crate) fn ceiling(peak: f32, floor: f32) -> f32 {
    let peak = peak.max(floor);
    let magnitude = 10.0_f32.powf(peak.log10().floor());
    let step = peak / magnitude;
    magnitude
        * if step <= 1.0 {
            1.0
        } else if step <= 2.0 {
            2.0
        } else if step <= 5.0 {
            5.0
        } else {
            10.0
        }
}
