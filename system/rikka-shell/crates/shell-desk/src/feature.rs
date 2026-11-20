use rikka_seam::SeamClient;
use shell_feeds::Feeds;
use shell_kit::{AimPin, Feature, KeyboardMode, Presence, Shell, SurfaceDesc, appearance};
use shell_wallpaper::ShownOn;

use crate::board::Board;
use crate::desk_scene::{DeskScene, Focus};
use crate::desk_surface::{self, DeskSurface};
use crate::edges::Edges;
use crate::menu;

#[derive(Debug)]
pub struct Desk {
    desk_scene: DeskScene,
    shown_on: ShownOn,
}

impl Desk {
    #[must_use]
    pub fn new(seam: SeamClient, feeds: Feeds, shown_on: ShownOn) -> Desk {
        Desk {
            desk_scene: DeskScene {
                seam,
                board: Board::load(),
                edges: Edges::default(),
                feeds,
                focus: Focus::default(),
                menu_pin: AimPin::default(),
                menu: Presence::default(),
            },
            shown_on,
        }
    }

    #[must_use]
    pub fn desk_scene(&self) -> DeskScene {
        self.desk_scene.clone()
    }
}

impl Feature for Desk {
    fn install(self, shell: Shell) -> Shell {
        let Desk {
            desk_scene,
            shown_on,
        } = self;
        let shell = desk_scene.edges.follow(shell);
        let shell = shell.follow(appearance::DOMAIN, appearance::WALLPAPER, {
            let board = desk_scene.board.clone();
            move |_, value: String| board.show(appearance::wallpaper_id(value))
        });
        let shell = install_desk_surface(shell, desk_scene.clone(), shown_on);
        menu::install(shell, desk_scene)
    }
}

fn install_desk_surface(shell: Shell, desk_scene: DeskScene, shown_on: ShownOn) -> Shell {
    shell.surface(
        SurfaceDesc::per_output(
            move |screen| DeskSurface::new(screen, shown_on.clone(), desk_scene.clone()),
            desk_surface::view,
        )
        .on_start(DeskSurface::start)
        .on_moved(DeskSurface::moved)
        .desktop()
        .keyboard(KeyboardMode::OnDemand),
    )
}
