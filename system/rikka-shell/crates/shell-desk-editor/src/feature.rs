use rikka_seam::Request;
use rikka_session::shortcuts::Shortcut;
use rikka_session::surfaces;
use rikka_ui::Inset;
use shell_desk::{CAPABILITY, DeskScene};
use shell_kit::taskbar::{Shape, TaskbarVisibility};
use shell_kit::{
    DesktopFrost, Feature, KeyboardMode, PointerMode, PresenceService, Reach, Shell, SurfaceDesc,
    TOGGLE,
};

use crate::shared::Shared;
use crate::stage::Editor;
use crate::{EXIT_DURATION, view};

#[derive(Debug)]
pub struct DeskEditor {
    desk_scene: DeskScene,
}

impl DeskEditor {
    #[must_use]
    pub fn new(desk_scene: DeskScene) -> DeskEditor {
        DeskEditor { desk_scene }
    }
}

impl Feature for DeskEditor {
    fn install(self, shell: Shell) -> Shell {
        let service = PresenceService::new(
            CAPABILITY,
            "The desktop's widgets, to arrange for each wallpaper",
            "the widget editor",
        );
        let presence = service.presence();
        let shell = shell
            .service(service)
            .on_shortcut(Shortcut::EditDesk, Request::new(CAPABILITY, TOGGLE));
        let taskbar_visibility = TaskbarVisibility::read(&shell);
        let reach = Reach::new(reaching(taskbar_visibility.shape()));
        let shell = taskbar_visibility.following(shell, {
            let (taskbar_visibility, reach) = (taskbar_visibility.clone(), reach.clone());
            move |_| {
                reach.set(reaching(taskbar_visibility.shape()));
            }
        });
        let frost = DesktopFrost::default();
        let shared = Shared::new(self.desk_scene, frost.clone());
        shell
            .surface(frost.surface(presence.clone(), EXIT_DURATION))
            .surface(
                SurfaceDesc::per_output(
                    move |place| Editor::new(shared.clone(), place),
                    view::view,
                )
                .on_start(Editor::start)
                .on_leave(Editor::left)
                .name(surfaces::DESK)
                .overlay()
                .keyboard(KeyboardMode::Exclusive)
                .reach(reach)
                .drives(frost.opacity())
                .animated(presence, EXIT_DURATION),
            )
    }
}

fn reaching(taskbar: Option<Shape>) -> PointerMode {
    match taskbar {
        Some(shape) => PointerMode::Inset(Inset::edges(shape.reserved(), 0.0, 0.0, 0.0)),
        None => PointerMode::Take,
    }
}
