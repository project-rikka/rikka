use rikka_seam::Request;
use rikka_session::shortcuts::Shortcut;
use shell_apps::Apps;
use shell_kit::{Feature, KeyboardMode, PresenceService, Shell, SurfaceDesc, TOGGLE, appearance};

use crate::apps::AppsProvider;
use crate::arrivals::Arrivals;
use crate::calc::CalcProvider;
use crate::commands::{Chosen, CommandsProvider, ThemeCommand, WallpaperCommand};
use crate::registry::Providers;
use crate::run::RunProvider;
use crate::state::Launcher;
use crate::web::WebProvider;
use crate::{CAPABILITY, view};

#[derive(Debug)]
pub struct Search {
    apps: Apps,
}

impl Search {
    #[must_use]
    pub fn new(apps: Apps) -> Search {
        Search { apps }
    }
}

impl Feature for Search {
    fn install(self, shell: Shell) -> Shell {
        let service = PresenceService::new(CAPABILITY, "The desktop's launcher", "the launcher");
        let presence = service.presence();
        let shell = shell.service(service);
        let seam = shell.seam();

        let theme = Chosen::default();
        let wallpaper = Chosen::default();
        let installed = self.apps.installed().clone();
        let arrivals = Arrivals::follow(&installed);
        let providers = Providers::new()
            .with(AppsProvider::new(self.apps, arrivals))
            .away(CalcProvider::default())
            .with(
                CommandsProvider::new()
                    .with(ThemeCommand::new(theme.clone()))
                    .with(WallpaperCommand::new(wallpaper.clone(), seam.clone())),
            )
            .with(RunProvider::default())
            .with(WebProvider::default());

        shell
            .follow(
                appearance::DOMAIN,
                appearance::THEME,
                move |_, slug: String| {
                    theme.set(slug);
                },
            )
            .follow(
                appearance::DOMAIN,
                appearance::WALLPAPER,
                move |_, id: String| {
                    wallpaper.set(id);
                },
            )
            .on_shortcut(Shortcut::ToggleLauncher, Request::new(CAPABILITY, TOGGLE))
            .surface(
                SurfaceDesc::new(
                    move || Launcher::new(seam.clone(), providers.clone(), installed.clone()),
                    view::view,
                )
                .on_start(Launcher::start)
                .overlay()
                .keyboard(KeyboardMode::Exclusive)
                .animated(presence, view::EXIT_DURATION),
            )
    }
}
