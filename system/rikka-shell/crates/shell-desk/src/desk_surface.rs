use desk_layout::Layout;
use rikka_ui::{Element, State};
use shell_kit::Placement;
use shell_wallpaper::{Decision, ShownOn};

use crate::desk_scene::DeskScene;
use crate::scene::{Scene, desktop};
use crate::widgets::{Live, Mode};

#[derive(Debug, State)]
pub(super) struct DeskSurface {
    screen: Placement,
    #[untracked]
    shown_on: ShownOn,
    #[state]
    live: Live,
}

impl DeskSurface {
    #[must_use]
    pub(crate) fn new(screen: Placement, shown_on: ShownOn, desk_scene: DeskScene) -> DeskSurface {
        DeskSurface {
            screen,
            shown_on,
            live: Live::new(desk_scene),
        }
    }

    pub(crate) fn start(&mut self) {
        self.live.start();
    }

    pub(crate) fn moved(&mut self, screen: Placement) {
        self.screen = screen;
    }
}

pub(crate) fn view(desk_surface: &DeskSurface) -> Element {
    let desk_scene = desk_surface.live.desk_scene();
    let output = desk_surface.screen.size;
    let (wallpaper, layout) = match desk_surface.shown_on.on(&desk_surface.screen.name) {
        Decision::Made(wallpaper) => {
            let layout = desk_scene.board.layout(wallpaper.as_deref());
            (wallpaper, layout)
        }
        Decision::Pending => (None, Layout::default()),
    };
    let scene = Scene {
        wallpaper,
        layout,
        output,
        area: desk_scene.area_of(output),
        mode: Mode::Live,
        grabbed: None,
    };
    desktop(scene, desk_scene, &desk_surface.screen)
}
