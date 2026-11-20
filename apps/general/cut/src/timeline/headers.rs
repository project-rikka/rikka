use cut_project::{Edit, Project, Switch};
use rikka_app::prelude::*;
use rikka_widgets::row;

use crate::controls::toggle;
use crate::cut::Cut;
use crate::timeline::rows::{Row, Rows};
use crate::timeline::sizing::{GRIP, Sizing};
use crate::wording;

pub(crate) const WIDTH: f32 = 132.0;
const GLYPH: f32 = 16.0;

pub(crate) fn headers(project: &Project, rows: &Rows, opening: Option<Element>) -> impl View {
    let expanded_layer = rows.opening().map(|opening| opening.layer);
    let mut opening = opening;
    let mut column = Vec::new();
    for row in rows.all() {
        column.push(header(project, row).build());
        if expanded_layer == Some(row.layer)
            && let Some(opening) = opening.take()
        {
            column.push(opening);
        }
    }
    column_of(column).width(WIDTH)
}

fn header(project: &Project, row: Row) -> impl View {
    let layer = project.layer(row.layer);
    let visible = layer.is_none_or(|layer| layer.is_shown());
    let locked = layer.is_some_and(|layer| layer.is_locked());
    let eye = match visible {
        true => MaterialSymbol::VISIBILITY,
        false => MaterialSymbol::VISIBILITY_OFF,
    };
    let lock = match locked {
        true => MaterialSymbol::LOCK,
        false => MaterialSymbol::LOCK_OPEN,
    };
    let index = row.layer;
    let controls = row! {
        text(wording::layer(index)).caption().no_wrap();
        spacer().grow(1.0);
        toggle(eye, GLYPH, !visible, move |cut: &mut Cut| {
            cut.edit(Edit::Set {
                layer: index,
                switch: Switch::Shown,
                on: !visible,
            })
        })
        .tooltip(match visible {
            true => tr!("cut.timeline.hide_layer"),
            false => tr!("cut.timeline.show_layer"),
        });
        toggle(lock, GLYPH, locked, move |cut: &mut Cut| {
            cut.edit(Edit::Set {
                layer: index,
                switch: Switch::Locked,
                on: !locked,
            })
        })
        .tooltip(match locked {
            true => tr!("cut.timeline.unlock_layer"),
            false => tr!("cut.timeline.lock_layer"),
        });
    }
    .gap(Space::TIGHT)
    .align_items(Align::Center)
    .padding(Inset::xy(Space::SNUG, Space::NONE))
    .fill();
    let sizing = Sizing {
        layer: index,
        height: row.height,
    }
    .build()
    .position(0.0, row.height - GRIP)
    .fill_width();
    container(column_of(vec![controls.build(), sizing]).fill())
        .background(palette().surface)
        .border(1.0, palette().outline)
        .height(row.height)
        .fill_width()
}
