use std::cell::Cell;
use std::rc::Rc;

use desk_layout::{Area, WidgetId};
use rikka_seam::{Request, SeamClient};
use rikka_types::{Point, Rect, Size};
use shell_feeds::Feeds;
use shell_kit::launch::Spawn;
use shell_kit::{AimPin, OPEN, Placement, Presence};

use crate::board::Board;
use crate::edges::Edges;
use crate::{CAPABILITY, MENU};

#[derive(Clone, Debug)]
pub struct DeskScene {
    pub seam: SeamClient,
    pub board: Board,
    pub edges: Edges,
    pub(crate) feeds: Feeds,
    pub focus: Focus,
    pub(crate) menu_pin: AimPin,
    pub(crate) menu: Presence,
}

#[derive(Clone, Debug, Default)]
pub struct Focus(Rc<Cell<Option<WidgetId>>>);

impl Focus {
    pub fn take(&self) -> Option<WidgetId> {
        self.0.take()
    }

    fn set(&self, widget: Option<WidgetId>) {
        self.0.set(widget);
    }
}

impl DeskScene {
    #[must_use]
    pub fn area_of(&self, output: Size) -> Area {
        self.edges.area(Rect::from_origin_size(Point::ZERO, output))
    }

    #[must_use]
    pub(crate) fn widget_at(&self, output: Size, at: Point) -> Option<WidgetId> {
        self.board
            .layout(self.board.showing().as_deref())
            .widget_at(&self.area_of(output), at)
            .map(|(widget, _)| widget)
    }

    pub(crate) fn edit(&self, widget: Option<WidgetId>) {
        self.focus.set(widget);
        self.seam.notify(Request::new(CAPABILITY, OPEN));
    }

    pub(super) fn offer_menu(&self, widget: Option<WidgetId>) {
        self.menu_pin.clear();
        self.open_menu(widget);
    }

    pub(super) fn offer_menu_at(&self, widget: Option<WidgetId>, screen: &Placement, at: Point) {
        self.menu_pin.point(screen, at);
        self.open_menu(widget);
    }

    fn open_menu(&self, widget: Option<WidgetId>) {
        self.focus.set(widget);
        self.seam.notify(Request::new(MENU, OPEN));
    }

    pub(crate) fn reopen_menu_at(&self, widget: Option<WidgetId>, screen: &Placement, at: Point) {
        self.menu_pin.point(screen, at);
        self.focus.set(widget);
        self.menu.reopen();
    }

    pub(crate) fn launch<S: Into<String>>(&self, argv: impl IntoIterator<Item = S>) {
        self.seam.notify(Spawn::new(argv).request());
    }
}
