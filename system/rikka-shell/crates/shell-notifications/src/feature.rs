use rikka_notifications::Notifier;
use rikka_seam::Request;
use rikka_ui::Inset;
use shell_apps::Apps;
use shell_kit::{Anchor, Feature, Layer, PointerMode, Presence, Shell, SurfaceDesc};

use crate::action_router::ActionRouter;
use crate::history::History;
use crate::quiet::Quiet;
use crate::service::{ARRIVED, CAPABILITY, NotifyService};
use crate::settings::{self, DO_NOT_DISTURB, TIMEOUT, TIMEOUT_DEFAULT, VISIBLE, VISIBLE_DEFAULT};
use crate::shared::{Preferences, Shared};
use crate::stack::Stack;
use crate::toasts::Toasts;
use crate::view;

const GAP: f32 = 16.0;

#[derive(Debug)]
pub struct NotificationCenter {
    apps: Apps,
    quiet: Quiet,
    history: History,
}

impl NotificationCenter {
    #[must_use]
    pub fn new(apps: Apps, quiet: Quiet, history: History) -> NotificationCenter {
        NotificationCenter {
            apps,
            quiet,
            history,
        }
    }
}

impl Feature for NotificationCenter {
    fn install(self, shell: Shell) -> Shell {
        let shell = shell.settings(settings::schema());
        let shell = shell.follow(settings::DOMAIN, DO_NOT_DISTURB, {
            let quiet = self.quiet.clone();
            move |_, enabled: bool| quiet.set(enabled)
        });
        let seam = shell.seam();
        let nudging = seam.clone();
        let notifier = match Notifier::start(move || {
            nudging.notify(Request::new(CAPABILITY, ARRIVED));
        }) {
            Ok(notifier) => notifier,
            Err(error) => {
                tracing::warn!("notification server unavailable: {error}");
                return shell;
            }
        };
        tracing::info!("serving org.freedesktop.Notifications");
        let action_router = ActionRouter::new(notifier, seam.clone());
        self.history.answers_with(action_router.clone());

        let presence = Presence::default();
        let shared = Shared {
            seam,
            stack: Stack::new(settings::shown(VISIBLE_DEFAULT), presence.clone()),
            history: self.history,
            action_router,
            preferences: Preferences::new(self.quiet, settings::linger(TIMEOUT_DEFAULT)),
            apps: self.apps,
        };
        let drawing = shared.clone();
        shell
            .service(NotifyService::new(shared.clone()))
            .follow(settings::DOMAIN, TIMEOUT, {
                let preferences = shared.preferences.clone();
                move |_, seconds: i64| preferences.stand_for(settings::linger(seconds))
            })
            .follow(settings::DOMAIN, VISIBLE, {
                let stack = shared.stack.clone();
                let seam = shared.seam.clone();
                move |_, count: i64| {
                    stack.show_at_most(settings::shown(count));
                    seam.notify(Request::new(CAPABILITY, ARRIVED));
                }
            })
            .surface(
                SurfaceDesc::new(move || Toasts::new(drawing.clone()), view::view)
                    .on_start(Toasts::start)
                    .name("notifications")
                    .layer(Layer::Top)
                    .anchor(Anchor::TOP | Anchor::RIGHT)
                    .width(view::SPAN)
                    .margin(Inset::edges(GAP, 0.0, 0.0, 0.0))
                    .pointer(PointerMode::Panes)
                    .presence(presence),
            )
    }
}
