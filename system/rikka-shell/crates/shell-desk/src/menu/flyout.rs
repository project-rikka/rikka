use desk_layout::WidgetId;
use rikka_seam::Request;
use rikka_session::settings_app::{Link, Section, Spot};
use rikka_types::{Point, Rect};
use rikka_ui::State;
use shell_kit::toplevels::Toplevels;
use shell_kit::{AimTarget, CLOSE, Placement};

use crate::MENU;
use crate::desk_scene::DeskScene;
use crate::menu::rows::{MenuAction, rows};

#[derive(Debug, State)]
pub(crate) struct Flyout {
    desk_scene: DeskScene,
    screen: Placement,
    widget: Option<WidgetId>,
    aim: Option<AimTarget>,
    #[state]
    windows: Toplevels,
}

impl Flyout {
    #[must_use]
    pub(crate) fn new(desk_scene: DeskScene, screen: Placement, aim: Option<AimTarget>) -> Flyout {
        Flyout {
            widget: desk_scene.focus.take(),
            desk_scene,
            screen,
            aim,
            windows: Toplevels::default(),
        }
    }

    pub(crate) fn start(&mut self) {
        self.windows.start(&self.desk_scene.seam);
    }

    #[must_use]
    pub(crate) fn aim(&self) -> Option<AimTarget> {
        self.aim
    }

    #[must_use]
    pub(super) fn on_widget(&self) -> bool {
        self.widget.is_some()
    }

    pub(crate) fn dismiss(&mut self) {
        self.desk_scene.seam.notify(Request::new(MENU, CLOSE));
    }

    pub(crate) fn reopen(&mut self, at: Point) {
        let desk_scene = &self.desk_scene;
        let output = Rect::from_origin_size(Point::ZERO, self.screen.size);
        if desk_scene.edges.taskbar_covers(output, at) || self.windows.covers(&self.screen, at) {
            self.dismiss();
            return;
        }
        let widget = desk_scene.widget_at(self.screen.size, at);
        desk_scene.reopen_menu_at(widget, &self.screen, at);
    }

    pub(crate) fn chose(&mut self, row: usize) {
        let Some(doing) = rows(self.on_widget())
            .into_iter()
            .nth(row)
            .map(|row| row.doing)
        else {
            return;
        };
        if doing == MenuAction::Nothing {
            return;
        }
        self.dismiss();
        match doing {
            MenuAction::Edit => self.desk_scene.edit(self.widget),
            MenuAction::Wallpaper => self.desk_scene.launch(Link::Spot(Spot::Wallpaper).argv()),
            MenuAction::Remove => {
                if let Some(widget) = self.widget {
                    let board = &self.desk_scene.board;
                    board.change(board.showing().as_deref(), |layout| {
                        layout.remove(widget);
                    });
                }
            }
            MenuAction::Settings => self
                .desk_scene
                .launch(Link::Section(Section::Appearance).argv()),
            MenuAction::Nothing => {}
        }
    }
}
