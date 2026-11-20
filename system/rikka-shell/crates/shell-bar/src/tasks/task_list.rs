use std::collections::HashMap;

use rikka_types::{Point, PointerButton, Size};
use rikka_ui::{Element, Handler, IntoHandler, View};
use rikka_widgets::row;
use task_board::Group;

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
    pub(crate) centre: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Touch {
    Aimed(Aim),
    Chose {
        space: usize,
        key: String,
        button: PointerButton,
        at: Point,
    },
}

#[must_use]
pub(crate) fn task_list<Marker>(
    groups: &[Group],
    space: usize,
    tiles: &HashMap<String, Tile>,
    widest: f32,
    on_touch: impl IntoHandler<(Touch,), (), Marker>,
) -> Element {
    let on_touch: Handler<(Touch,)> = on_touch.into_handler();
    let shown: Vec<(&Group, &Tile)> = groups
        .iter()
        .filter_map(|group| Some((group, tiles.get(&group.key)?)))
        .collect();
    let squeeze = squeeze(shown.len(), widest);
    let size = Size::new(CELL * squeeze, HEIGHT);
    row! {
        for (group, tile) in shown {
            glide(
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
            );
        }
    }
    .gap(CELL_GAP * squeeze)
    .build()
}

fn squeeze(count: usize, widest: f32) -> f32 {
    let natural = count as f32 * CELL + count.saturating_sub(1) as f32 * CELL_GAP;
    match natural > widest && natural > 0.0 {
        true => (widest / natural).max(SQUEEZED),
        false => 1.0,
    }
}
