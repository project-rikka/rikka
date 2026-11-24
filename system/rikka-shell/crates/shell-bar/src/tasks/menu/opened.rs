use std::cell::RefCell;
use std::rc::Rc;

use rikka_settings::session::appearance::TaskbarPosition;
use rikka_types::Point;
use rikka_ui::AnchorEdge;
use shell_kit::launch::Spawn;
use shell_kit::taskbar::Edge;
use shell_kit::{Destination, Presence};
use task_board::{Group, Window};

use crate::tasks::tile::Tile;

#[derive(Clone, Debug)]
pub(crate) struct MenuRequest {
    pub(crate) at: Point,
    pub(crate) edge: Edge,
    pub(crate) space: usize,
    pub(crate) name: String,
    pub(crate) windows: Vec<Window>,
    pub(crate) starts: Option<Spawn>,
}

impl MenuRequest {
    #[must_use]
    pub(crate) fn new(
        at: Point,
        edge: Edge,
        space: usize,
        group: &Group,
        tile: &Tile,
    ) -> MenuRequest {
        MenuRequest {
            at,
            edge,
            space,
            name: tile.name.clone(),
            windows: group.windows.clone(),
            starts: tile.starts(),
        }
    }

    #[must_use]
    pub(crate) fn toward(&self) -> AnchorEdge {
        match self.edge.position() {
            TaskbarPosition::Top => AnchorEdge::BelowStart,
            TaskbarPosition::Bottom => AnchorEdge::AboveStart,
            TaskbarPosition::Left => AnchorEdge::BesideTop,
            TaskbarPosition::Right => AnchorEdge::BesideLeftTop,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Opened {
    presence: Presence,
    destination: Destination,
    request: Rc<RefCell<Option<MenuRequest>>>,
}

impl Opened {
    #[must_use]
    pub(crate) fn presence(&self) -> Presence {
        self.presence.clone()
    }

    #[must_use]
    pub(crate) fn destination(&self) -> Destination {
        self.destination.clone()
    }

    #[must_use]
    pub(crate) fn request(&self) -> Option<MenuRequest> {
        self.request.borrow().clone()
    }

    pub(crate) fn open(&self, output: &str, request: MenuRequest) {
        self.destination.aim(output);
        *self.request.borrow_mut() = Some(request);
        self.presence.set(true);
    }

    pub(crate) fn close(&self) {
        self.presence.set(false);
    }
}
