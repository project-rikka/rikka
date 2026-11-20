mod flyout;
mod rows;
mod view;

use shell_kit::{EXIT_DURATION, PresenceService, Shell, SurfaceDesc};

use crate::MENU;
use crate::desk_scene::DeskScene;
use crate::menu::flyout::Flyout;

const WIDTH: f32 = 304.0;

const NAMESPACE: &str = "desk-menu";

#[must_use]
pub(crate) fn install(shell: Shell, desk_scene: DeskScene) -> Shell {
    let service = PresenceService::new(
        MENU,
        "What can be done with the desktop or one of its widgets",
        "the desktop menu",
    )
    .with_presence(desk_scene.menu.clone());
    let presence = service.presence();
    let shell = shell.service(service);
    let aimed = shell.aimed().pinned(desk_scene.menu_pin.clone());
    shell.surface(
        SurfaceDesc::aimed(
            aimed,
            move |screen, aim| Flyout::new(desk_scene.clone(), screen, aim),
            view::view,
        )
        .on_start(Flyout::start)
        .name(NAMESPACE)
        .modal(presence, EXIT_DURATION),
    )
}
