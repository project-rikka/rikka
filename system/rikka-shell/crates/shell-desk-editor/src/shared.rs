use std::cell::Cell;
use std::rc::Rc;

use desk_layout::WidgetId;
use rikka_seam::{Request, SeamClient};
use shell_desk::{CAPABILITY, DeskScene};
use shell_kit::{CLOSE, DesktopFrost};

use crate::editing::Editing;

#[derive(Clone, Debug)]
pub(crate) struct Shared {
    pub(crate) desk_scene: DeskScene,
    pub(crate) editing: Editing,
    pub(crate) frost: DesktopFrost,
    open_widget: Rc<Cell<Option<WidgetId>>>,
}

impl Shared {
    #[must_use]
    pub(crate) fn new(desk_scene: DeskScene, frost: DesktopFrost) -> Shared {
        Shared {
            editing: Editing::new(desk_scene.board.clone(), desk_scene.seam.clone()),
            desk_scene,
            frost,
            open_widget: Rc::default(),
        }
    }

    #[must_use]
    pub(crate) fn seam(&self) -> &SeamClient {
        &self.desk_scene.seam
    }

    #[must_use]
    pub(crate) fn focused(&self) -> Option<WidgetId> {
        let focus = self.desk_scene.focus.take().or(self.open_widget.get());
        self.open_widget.set(focus);
        focus
    }

    pub(crate) fn left(&self) {
        self.open_widget.set(None);
        self.editing.history().forget();
    }

    pub(crate) fn close(&self) {
        self.seam().notify(Request::new(CAPABILITY, CLOSE));
    }
}
