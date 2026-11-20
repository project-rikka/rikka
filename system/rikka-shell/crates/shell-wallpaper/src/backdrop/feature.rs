use rikka_seam::Delivery;
use rikka_session::wallpaper::WallpaperClient;
use rikka_settings::seam as settings_seam;
use shell_kit::taskbar::{Shape, TaskbarVisibility};
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
    fn reserving(&self, shell: &Shell) -> f32 {
        let taskbar_visibility = self.taskbar_visibility(shell);
        reserved(
            taskbar_visibility.and_then(|taskbar_visibility| taskbar_visibility.shape()),
            &shell.shared_theme().theme(),
        )
    }

    fn taskbar_visibility(&self, shell: &Shell) -> Option<TaskbarVisibility> {
        self.beside_taskbar.then(|| TaskbarVisibility::read(shell))
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

        let taskbar_visibility = self.taskbar_visibility(&shell);
        let shell = match taskbar_visibility {
            Some(taskbar_visibility) => following_taskbar(shell, below.clone(), taskbar_visibility),
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
    below: Tracked<f32>,
    taskbar_visibility: TaskbarVisibility,
) -> Shell {
    let (shared_theme, wallpapers) = (shell.shared_theme(), WallpaperClient::new(shell.seam()));

    let shell = taskbar_visibility.following(shell, {
        let (taskbar_visibility, below, shared_theme, wallpapers) = (
            taskbar_visibility.clone(),
            below.clone(),
            shared_theme.clone(),
            wallpapers.clone(),
        );
        move |_| {
            publish(
                &wallpapers,
                &shared_theme,
                &below,
                taskbar_visibility.shape(),
            )
        }
    });

    shell.watch(settings_seam::CAPABILITY, move |_, incoming| {
        let redressed = match incoming {
            Delivery::Event(event) => appearance::redressed(event),
            Delivery::Lagged(_) => true,
        };
        if redressed {
            publish(
                &wallpapers,
                &shared_theme,
                &below,
                taskbar_visibility.shape(),
            );
        }
    })
}

fn publish(
    wallpapers: &WallpaperClient,
    shared_theme: &SharedTheme,
    below: &Tracked<f32>,
    taskbar: Option<Shape>,
) {
    let reserved = reserved(taskbar, &shared_theme.theme());
    if below.set(reserved) {
        wallpapers.reserve(reserved);
    }
}
