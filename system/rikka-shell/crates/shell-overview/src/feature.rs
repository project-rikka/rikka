use std::cell::Cell;

use rikka_seam::{Delivery, Request, SeamClient};
use rikka_session::gestures::{self, Axis};
use rikka_session::shortcuts::Shortcut;
use rikka_session::surfaces;
use rikka_ui::Inset;
use shell_desk::DeskScene;
use shell_kit::taskbar::TaskbarVisibility;
use shell_kit::{
    CLOSE, DesktopFrost, Feature, KeyboardMode, OPEN, PointerMode, Presence, PresenceService,
    Reach, Shell, SurfaceDesc, TOGGLE, appearance,
};

use crate::desktop::Desktop;
use crate::shared::Shared;
use crate::stage::{self, Stage};
use crate::swipe::{SwipeState, Swipes};
use crate::{CAPABILITY, view};

#[derive(Debug)]
pub struct Overview {
    desk_scene: DeskScene,
}

impl Overview {
    #[must_use]
    pub fn new(desk_scene: DeskScene) -> Overview {
        Overview { desk_scene }
    }
}

impl Feature for Overview {
    fn install(self, shell: Shell) -> Shell {
        let service = PresenceService::new(
            CAPABILITY,
            "Every window and workspace of a display at once",
            "the overview",
        );
        let presence = service.presence();
        let shell = shell.service(service);

        let desktop = Desktop::default();
        let taskbar_visibility = TaskbarVisibility::read(&shell);
        desktop.set_taskbar(taskbar_visibility.shape());
        let reach = Reach::new(reaching(&desktop));

        let shell = taskbar_visibility.following(shell, {
            let (taskbar_visibility, desktop, reach) =
                (taskbar_visibility.clone(), desktop.clone(), reach.clone());
            move |_| {
                desktop.set_taskbar(taskbar_visibility.shape());
                reach.set(reaching(&desktop));
            }
        });

        let frost = DesktopFrost::default();
        let shared = Shared {
            seam: shell.seam(),
            desktop: desktop.clone(),
            swipes: Swipes::default(),
            desk_scene: self.desk_scene,
            frost: frost.clone(),
        };
        let swiping = Swiping {
            seam: shell.seam(),
            swipes: shared.swipes.clone(),
            presence: presence.clone(),
            last_request: Cell::new(None),
        };
        shell
            .follow(appearance::DOMAIN, appearance::WALLPAPER, {
                let desktop = desktop.clone();
                move |_, value: String| {
                    desktop.set_wallpaper(shell_wallpaper::file_of_setting(value))
                }
            })
            .on_shortcut(Shortcut::ToggleOverview, Request::new(CAPABILITY, TOGGLE))
            .watch(gestures::CAPABILITY, move |_, incoming| {
                swiping.handle_swipe(incoming)
            })
            .surface(frost.surface(presence.clone(), stage::EXIT_DURATION))
            .surface(
                SurfaceDesc::per_output(move |place| Stage::new(shared.clone(), place), view::view)
                    .on_start(Stage::start)
                    .name(surfaces::OVERVIEW)
                    .overlay()
                    .keyboard(KeyboardMode::Exclusive)
                    .reach(reach)
                    .drives(frost.opacity())
                    .animated(presence, stage::EXIT_DURATION),
            )
    }
}

struct Swiping {
    seam: SeamClient,
    swipes: Swipes,
    presence: Presence,
    last_request: Cell<Option<(u64, &'static str)>>,
}

impl Swiping {
    fn handle_swipe(&self, incoming: &Delivery) {
        let Some(swipe) = incoming.event().and_then(gestures::swiped) else {
            return;
        };
        let shown = self.presence.is_shown();
        let swipe_request = match self.swipes.handle_swipe(swipe, shown) {
            SwipeState::Tracking(track) if track.axis == Axis::Vertical => {
                (!shown && track.travel > 0.0).then_some((track.id, OPEN))
            }
            SwipeState::Released(track) if track.axis == Axis::Vertical => {
                match (track.stays_open(), shown) {
                    (true, false) => Some((track.id, OPEN)),
                    (false, true) => Some((track.id, CLOSE)),
                    _ => None,
                }
            }
            _ => None,
        };
        if let Some(request) = swipe_request
            && self.last_request.replace(Some(request)) != Some(request)
        {
            self.seam.notify(Request::new(CAPABILITY, request.1));
        }
    }
}

#[must_use]
fn reaching(desktop: &Desktop) -> PointerMode {
    match desktop.taskbar() {
        Some(shape) => PointerMode::Inset(Inset::edges(shape.reserved(), 0.0, 0.0, 0.0)),
        None => PointerMode::Take,
    }
}
