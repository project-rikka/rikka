use rikka_seam::Delivery;
use rikka_session::wallpaper::{Reserved, WallpaperClient};
use rikka_settings::seam as settings_seam;
use shell_kit::taskbar::{Shape, TaskbarLayout};
use shell_kit::{Feature, SharedTheme, Shell, SurfaceDesc, Tracked, appearance};

use crate::backdrop::canvas::{self, Wallpaper};
use crate::backdrop::reserved;
use crate::backdrop::service::WallpaperService;
use crate::backdrop::shown_on::ShownOn;

#[derive(Debug)]
pub struct Backdrop {
    beside_taskbar: bool,
    shown_on: ShownOn,
}

impl Backdrop {
    #[must_use]
    pub fn beside_taskbar() -> Backdrop {
        Backdrop {
            beside_taskbar: true,
            shown_on: ShownOn::default(),
        }
    }

    #[must_use]
    pub fn alone() -> Backdrop {
        Backdrop {
            beside_taskbar: false,
            shown_on: ShownOn::default(),
        }
    }

    #[must_use]
    pub fn shown_on(&self) -> ShownOn {
        self.shown_on.clone()
    }

    #[must_use]
    fn reserving(&self, shell: &Shell) -> Reserved {
        let taskbar_layout = self.taskbar_layout(shell);
        reserved(
            taskbar_layout.and_then(|taskbar_layout| taskbar_layout.shape()),
            &shell.shared_theme().theme(),
        )
    }

    fn taskbar_layout(&self, shell: &Shell) -> Option<TaskbarLayout> {
        self.beside_taskbar.then(|| TaskbarLayout::read(shell))
    }
}

impl Feature for Backdrop {
    fn install(self, shell: Shell) -> Shell {
        let below = Tracked::default();
        below.set(self.reserving(&shell));
        let service = WallpaperService::new()
            .below(below.get())
            .showing(appearance::wallpaper(&shell));
        let shell = shell.service(service);

        let wallpapers = WallpaperClient::new(shell.seam());
        let shell = shell.on_setting(
            appearance::DOMAIN,
            appearance::WALLPAPER,
            move |_, value: String| {
                let id = appearance::wallpaper_id(value);
                match &id {
                    Some(id) => tracing::info!("wallpaper set to {id}"),
                    None => tracing::info!("wallpaper cleared"),
                }
                wallpapers.show(id);
            },
        );

        let taskbar_layout = self.taskbar_layout(&shell);
        let shell = match taskbar_layout {
            Some(taskbar_layout) => following_taskbar(shell, below.clone(), taskbar_layout),
            None => shell,
        };

        let (seam, shown_on) = (shell.seam(), self.shown_on);
        shell.surface(
            SurfaceDesc::per_output(
                move |screen| {
                    Wallpaper::new()
                        .on(screen)
                        .following(seam.clone())
                        .below(below.get())
                        .shown_on(shown_on.clone())
                },
                canvas::view,
            )
            .on_start(Wallpaper::start)
            .on_moved(Wallpaper::moved)
            .wallpaper(),
        )
    }
}

fn following_taskbar(
    shell: Shell,
    below: Tracked<Reserved>,
    taskbar_layout: TaskbarLayout,
) -> Shell {
    let (shared_theme, wallpapers) = (shell.shared_theme(), WallpaperClient::new(shell.seam()));

    let shell = taskbar_layout.following(shell, {
        let (below, shared_theme, wallpapers) =
            (below.clone(), shared_theme.clone(), wallpapers.clone());
        move |shape| publish(&wallpapers, &shared_theme, &below, shape)
    });

    shell.watch(settings_seam::CAPABILITY, move |_, incoming| {
        let redressed = match incoming {
            Delivery::Event(event) => appearance::redressed(event),
            Delivery::Lagged(_) => true,
        };
        if redressed {
            publish(&wallpapers, &shared_theme, &below, taskbar_layout.shape());
        }
    })
}

fn publish(
    wallpapers: &WallpaperClient,
    shared_theme: &SharedTheme,
    below: &Tracked<Reserved>,
    taskbar: Option<Shape>,
) {
    let reserved = reserved(taskbar, &shared_theme.theme());
    if below.set(reserved) {
        wallpapers.reserve(reserved);
    }
}
