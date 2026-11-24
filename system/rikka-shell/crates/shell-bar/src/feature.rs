use rikka_settings::seam as settings_seam;
use rikka_settings::session::appearance::TaskbarPosition;
use rikka_settings::session::power;
use rikka_tray::Tray;
use rikka_ui::State;
use shell_apps::Apps;
use shell_feeds::{Daemons, Feeds, ScreenRecording};
use shell_ime::InputMethods;
use shell_kit::taskbar::{Edge, Shape, TaskbarLayout};
use shell_kit::{
    EXIT_DURATION as MENU_EXIT_DURATION, Feature, KeyboardMode, Layer, Presence, Shell,
    SurfaceDesc, appearance,
};
use shell_notifications::{History, Quiet};

use crate::centre::{self, Centre};
use crate::indicators::{self, Drawer, Shelf, Typing, Usage};
use crate::rim::EXIT_DURATION;
use crate::shared::Shared;
use crate::state::Bar;
use crate::status::Percentage;
use crate::tasks::{self, Flyout as MenuFlyout, Opened};
use crate::tray::{self, Flyout, Fold};
use crate::{dropdown_panel, frame, strip};

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
        let taskbar_layout = TaskbarLayout::read(&shell);
        let docks = Docks::default();
        docks.stand(taskbar_layout.shape());
        let shell = taskbar_layout.following(shell, {
            let docks = docks.clone();
            move |shape| docks.stand(shape)
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
            taskbar_layout,
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
            });
        let shell = Edge::ALL.into_iter().fold(shell, |shell, edge| {
            let shell = shell.surface(docks.docked(edge, bar(shared.clone(), edge)));
            frame::surfaces(edge)
                .into_iter()
                .fold(shell, |shell, desc| shell.surface(docks.docked(edge, desc)))
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

#[derive(Clone, Debug, Default)]
struct Docks {
    top: Presence,
    bottom: Presence,
    left: Presence,
    right: Presence,
}

impl Docks {
    fn stand(&self, shape: Option<Shape>) {
        for edge in Edge::ALL {
            self.presence(edge)
                .set(shape.is_some_and(|shape| shape.edge() == edge));
        }
    }

    fn presence(&self, edge: Edge) -> Presence {
        match edge.position() {
            TaskbarPosition::Top => &self.top,
            TaskbarPosition::Bottom => &self.bottom,
            TaskbarPosition::Left => &self.left,
            TaskbarPosition::Right => &self.right,
        }
        .clone()
    }

    fn docked<S: State>(&self, edge: Edge, desc: SurfaceDesc<S>) -> SurfaceDesc<S> {
        Edge::ALL
            .into_iter()
            .filter(|other| *other != edge)
            .fold(desc, |desc, other| desc.after(self.presence(other)))
            .animated(self.presence(edge), EXIT_DURATION)
    }
}

fn bar(shared: Shared, edge: Edge) -> SurfaceDesc<Bar> {
    let shape = Shape::new(edge.position());
    let desc = SurfaceDesc::per_output(
        move |placement| Bar::new(shared.clone(), &placement, shape),
        strip::view,
    )
    .on_start(Bar::start)
    .on_moved(Bar::moved)
    .name(NAME)
    .layer(Layer::Top)
    .anchor(edge.anchor())
    .exclusive_zone(shape.exclusive())
    .pointer(shape.pointer());
    match edge.is_vertical() {
        true => desc.width(shape.extent()),
        false => desc.height(shape.extent()),
    }
}
