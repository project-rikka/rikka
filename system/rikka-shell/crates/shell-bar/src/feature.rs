use rikka_settings::seam as settings_seam;
use rikka_settings::session::power;
use rikka_tray::Tray;
use shell_apps::Apps;
use shell_feeds::{Daemons, Feeds, ScreenRecording};
use shell_ime::InputMethods;
use shell_kit::taskbar::{Shape, TaskbarVisibility};
use shell_kit::{
    Anchor, EXIT_DURATION as MENU_EXIT_DURATION, Feature, KeyboardMode, Layer, Presence, Shell,
    SurfaceDesc, appearance,
};
use shell_notifications::{History, Quiet};

use crate::centre::{self, Centre};
use crate::frame::{self, Side};
use crate::indicators::{self, Drawer, Shelf, Typing, Usage};
use crate::shared::Shared;
use crate::state::Bar;
use crate::status::Percentage;
use crate::tasks::{self, Flyout as MenuFlyout, Opened};
use crate::tray::{self, Flyout, Fold};
use crate::{dropdown_panel, strip};

const NAME: &str = "taskbar";
#[derive(Debug)]
pub struct Taskbar {
    pub daemons: Daemons,
    pub feeds: Feeds,
    pub tray: Tray,
    pub apps: Apps,
    pub quiet: Quiet,
    pub history: History,
    pub recording: ScreenRecording,
    pub methods: InputMethods,
}

impl Feature for Taskbar {
    fn install(self, shell: Shell) -> Shell {
        let taskbar_visibility = TaskbarVisibility::read(&shell);
        let up = Presence::default();
        up.set(taskbar_visibility.is_up());
        let shell = taskbar_visibility.following(shell, {
            let up = up.clone();
            move |stands| {
                up.set(stands);
            }
        });
        let feeds = self.feeds;
        let usage = Usage::load();
        let shelf = Shelf::new(
            shell.seam(),
            indicators::all(Typing::new(self.methods), usage.clone()),
        );
        let shared = Shared {
            seam: shell.seam(),
            daemons: self.daemons,
            tray: self.tray,
            apps: self.apps,
            centre: Presence::default(),
            fold: Fold::default(),
            quiet: self.quiet,
            history: self.history,
            recording: self.recording,
            taskbar_visibility,
            shelf: shelf.clone(),
            percentage: Percentage::default(),
            feeds,
            menu: Opened::default(),
        };
        let centre = shared.centre.clone();
        let unfolded = shared.fold.presence();
        let drawn = shelf.presence();
        let menu = shared.menu.clone();
        let seam = shell.seam();

        let shell = shell
            .follow(appearance::DOMAIN, appearance::TASKBAR_INDICATORS, {
                let shelf = shelf.clone();
                move |_, kinds: Vec<String>| shelf.show(&kinds)
            })
            .follow(power::DOMAIN, power::BATTERY_PERCENTAGE, {
                let percentage = shared.percentage.clone();
                move |_, shown: bool| percentage.set(shown)
            })
            .watch(settings_seam::CAPABILITY, move |_, event| {
                usage.event(event)
            })
            .surface(bar(shared.clone(), up.clone()));
        let shell = Side::ARRANGED.into_iter().fold(shell, |shell, side| {
            shell.surface(frame::surface(side, up.clone()))
        });
        shell
            .surface(
                SurfaceDesc::destined(
                    menu.destination(),
                    {
                        let menu = menu.clone();
                        move |_| MenuFlyout::new(seam.clone(), &menu)
                    },
                    tasks::menu_view,
                )
                .name("taskbar-menu")
                .overlay()
                .keyboard(KeyboardMode::Exclusive)
                .animated(menu.presence(), MENU_EXIT_DURATION),
            )
            .surface(
                SurfaceDesc::new(
                    {
                        let shared = shared.clone();
                        move || Flyout::new(&shared)
                    },
                    tray::flyout,
                )
                .on_start(Flyout::start)
                .name("tray")
                .overlay()
                .keyboard(KeyboardMode::Exclusive)
                .animated(unfolded, dropdown_panel::EXIT_DURATION),
            )
            .surface(
                SurfaceDesc::new(move || Centre::new(&shared), centre::view)
                    .on_start(Centre::start)
                    .name("control-centre")
                    .overlay()
                    .keyboard(KeyboardMode::Exclusive)
                    .animated(centre, centre::EXIT_DURATION),
            )
            .surface(
                SurfaceDesc::new(move || Drawer::new(&shelf), indicators::panel)
                    .on_start(Drawer::start)
                    .name("indicator")
                    .overlay()
                    .keyboard(KeyboardMode::Exclusive)
                    .animated(drawn, dropdown_panel::EXIT_DURATION),
            )
    }
}

fn bar(shared: Shared, up: Presence) -> SurfaceDesc<Bar> {
    SurfaceDesc::per_output(
        move |placement| Bar::new(shared.clone(), &placement),
        strip::view,
    )
    .on_start(Bar::start)
    .on_moved(Bar::moved)
    .name(NAME)
    .layer(Layer::Top)
    .anchor(Anchor::TOP)
    .height(Shape.height())
    .exclusive_zone(Shape.exclusive())
    .pointer(Shape.pointer())
    .presence(up)
}
