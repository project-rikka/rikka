use std::collections::HashMap;

use rikka_types::{PointerButton, Rect};
use rikka_ui::{Axis, Element, Handler, IntoHandler, View};
use task_board::Group;

use crate::axis;
use crate::glide::{glide, grows};
use crate::tasks::task_icon::TaskIcon;
use crate::tasks::tile::Tile;

const HEIGHT: f32 = 28.0;
const CELL: f32 = 28.0;
const CELL_GAP: f32 = 2.0;
const SQUEEZED: f32 = 0.5;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Aim {
    pub(crate) space: usize,
    pub(crate) key: String,
    pub(crate) bounds: Rect,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Touch {
    Aimed(Aim),
    Chose {
        space: usize,
        key: String,
        button: PointerButton,
        bounds: Rect,
    },
}

pub(crate) struct TaskList<'a> {
    pub(crate) groups: &'a [Group],
    pub(crate) space: usize,
    pub(crate) tiles: &'a HashMap<String, Tile>,
    pub(crate) longest: f32,
    pub(crate) axis: Axis,
}

#[must_use]
pub(crate) fn task_list<Marker>(
    list: TaskList<'_>,
    on_touch: impl IntoHandler<(Touch,), (), Marker>,
) -> Element {
    let TaskList {
        groups,
        space,
        tiles,
        longest,
        axis,
    } = list;
    let on_touch: Handler<(Touch,)> = on_touch.into_handler();
    let shown: Vec<(&Group, &Tile)> = groups
        .iter()
        .filter_map(|group| Some((group, tiles.get(&group.key)?)))
        .collect();
    let squeeze = squeeze(shown.len(), longest);
    let size = axis::size(axis, CELL * squeeze, HEIGHT);
    axis::line(
        axis,
        shown.into_iter().map(|(group, tile)| {
            glide(
                axis,
                TaskIcon {
                    space,
                    key: group.key.clone(),
                    icon: tile.icon.clone(),
                    minimized: group.is_minimized(),
                    size,
                    on_touch: on_touch.clone(),
                },
                grows(),
                (space, group.key.clone()),
            )
        }),
    )
    .gap(CELL_GAP * squeeze)
    .build()
}

fn squeeze(count: usize, longest: f32) -> f32 {
    let natural = count as f32 * CELL + count.saturating_sub(1) as f32 * CELL_GAP;
    match natural > longest && natural > 0.0 {
        true => (longest / natural).max(SQUEEZED),
        false => 1.0,
    }
}
