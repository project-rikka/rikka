use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use desk_layout::{Content, WidgetId};
use rikka_mpris::{Control, PlayerId};
use rikka_sysmon::Vitals;
use rikka_ui::{State, after_keyed, cancel, spawn_blocking, stream, stream_keyed};
use shell_kit::{Need, Period};

use crate::desk_scene::DeskScene;
use crate::widgets::needs::Needs;
use crate::widgets::sources::{Listening, Sources};

const WEATHER: &str = "desk.widgets.weather";
const PLAYER: &str = "desk.widgets.player";
const MACHINE: &str = "desk.widgets.machine";
const BATTERY: &str = "desk.widgets.battery";
const BLUETOOTH: &str = "desk.widgets.bluetooth";
const KEEPING: &str = "desk.widgets.note";

const KEEP_AFTER: Duration = Duration::from_millis(600);

#[derive(Debug, State)]
pub struct Live {
    #[untracked]
    desk_scene: DeskScene,
    #[untracked]
    sources: Rc<Sources>,
    #[untracked]
    everything: bool,
    #[untracked]
    following: Needs,
    #[untracked]
    cadence: Option<(Period, Need)>,
    #[untracked]
    listening: Option<Listening>,
}

impl Live {
    #[must_use]
    pub fn new(desk_scene: DeskScene) -> Live {
        let sources = Sources::shared(&desk_scene.feeds);
        Live {
            desk_scene,
            sources,
            everything: false,
            following: Needs::default(),
            cadence: None,
            listening: None,
        }
    }

    #[must_use]
    pub fn everything(mut self) -> Live {
        self.everything = true;
        self
    }

    pub fn start(&mut self) {
        stream(
            self.desk_scene.board.changes(),
            |live: &mut Live, _: u64| {
                live.arrange();
            },
        );
        let scale = &self.desk_scene.feeds.scale;
        self.sources.unit.set(scale.get());
        stream(scale.changes(), |live: &mut Live, unit| {
            live.sources.unit.set(unit)
        });
        self.arrange();
    }

    #[must_use]
    pub(crate) fn desk_scene(&self) -> &DeskScene {
        &self.desk_scene
    }

    #[must_use]
    pub(crate) fn sources(&self) -> &Rc<Sources> {
        &self.sources
    }

    pub(crate) fn control(&mut self, player: &PlayerId, control: Control) {
        self.desk_scene
            .feeds
            .services
            .media()
            .control(player, control);
    }

    pub(super) fn turn_page(&mut self, widget: WidgetId, by: i32) {
        self.sources.pages.alter(|pages| {
            *pages.entry(widget).or_default() += by;
            true
        });
    }

    pub(super) fn back_to_today(&mut self, widget: WidgetId) {
        self.sources
            .pages
            .alter(|pages| pages.remove(&widget).is_some_and(|page| page != 0));
    }

    pub(crate) fn show(&mut self, picture: PathBuf) {
        let target = rikka_open::path_to_uri(&picture);
        spawn_blocking(
            move || rikka_open::open(&target),
            move |_: &mut Live, result: rikka_open::Result<()>| {
                if let Err(error) = result {
                    tracing::warn!("failed to open {}: {error}", picture.display());
                }
            },
        );
    }

    pub(crate) fn typed(&mut self, note: WidgetId) {
        let showing = self.desk_scene.board.showing();
        self.sources.drafts.typed(note, showing);
        after_keyed((KEEPING, note), KEEP_AFTER, move |live: &mut Live| {
            live.keep(note);
        });
    }

    pub(crate) fn keep(&mut self, note: WidgetId) {
        cancel((KEEPING, note));
        let Some(unkept) = self.sources.drafts.unkept(note) else {
            return;
        };
        self.desk_scene
            .board
            .change(unkept.wallpaper.as_deref(), |layout| {
                if let Some(Content::Note(edited_note)) =
                    layout.get_mut(note).map(|widget| &mut widget.content)
                {
                    edited_note.text = unkept.text;
                }
            });
    }

    fn arrange(&mut self) {
        let layout = self.desk_scene.board.shown_layout();
        self.sources.drafts.settle(&layout);
        let needs = match self.everything {
            true => Needs::EVERYTHING,
            false => Needs::for_layout(&layout),
        };
        self.follow(needs);
    }

    fn follow(&mut self, needs: Needs) {
        let was = std::mem::replace(&mut self.following, needs);
        if needs.weather != was.weather {
            self.follow_weather(needs.weather);
        }
        if needs.player != was.player {
            self.follow_player(needs.player);
        }
        if needs.machine != was.machine {
            self.follow_machine(needs.machine);
        }
        if needs.power != was.power {
            self.follow_battery(needs.power);
        }
        let devices = needs.power && needs.devices;
        if devices != (was.power && was.devices) {
            self.follow_bluetooth(devices);
        }
        self.listen();
        self.retime();
    }

    fn follow_weather(&mut self, on: bool) {
        if !on {
            return cancel(WEATHER);
        }
        let weather = self.desk_scene.feeds.services.weather();
        self.sources.weather.set(weather.state());
        stream_keyed(
            WEATHER,
            weather.changes(),
            |live: &mut Live, state: rikka_weather::State| live.sources.weather.set(state),
        );
    }

    fn follow_player(&mut self, on: bool) {
        if !on {
            return cancel(PLAYER);
        }
        let players = self.desk_scene.feeds.services.media();
        self.sources.player.set(players.state().active().cloned());
        stream_keyed(PLAYER, players.changes(), Live::on_player);
    }

    fn follow_machine(&mut self, on: bool) {
        if !on {
            return cancel(MACHINE);
        }
        let machine = self.desk_scene.feeds.services.machine();
        let vitals = machine.state();
        self.sources.machine.alter(|history| history.record(vitals));
        stream_keyed(
            MACHINE,
            machine.changes(),
            |live: &mut Live, vitals: Arc<Vitals>| {
                live.sources.machine.alter(|history| history.record(vitals));
            },
        );
    }

    fn follow_battery(&mut self, on: bool) {
        if !on {
            return cancel(BATTERY);
        }
        let battery = self.desk_scene.feeds.services.battery();
        self.sources.charging.on_battery(battery.state());
        stream_keyed(
            BATTERY,
            battery.changes(),
            |live: &mut Live, state: rikka_battery::State| live.sources.charging.on_battery(state),
        );
    }

    fn follow_bluetooth(&mut self, on: bool) {
        if !on {
            return cancel(BLUETOOTH);
        }
        let bluetooth = self.desk_scene.feeds.services.bluetooth();
        self.sources.charging.on_bluetooth(&bluetooth.state());
        stream_keyed(
            BLUETOOTH,
            bluetooth.changes(),
            |live: &mut Live, state: rikka_bluetooth::State| {
                live.sources.charging.on_bluetooth(&state);
            },
        );
    }

    fn on_player(&mut self, state: rikka_mpris::State) {
        self.sources.player.set(state.active().cloned());
        self.listen();
        self.retime();
    }

    fn playing(&self) -> bool {
        self.sources
            .player
            .with(|player| player.as_ref().is_some_and(rikka_mpris::Player::is_playing))
    }

    fn listen(&mut self) {
        let should_listen = self.following.waveform && self.playing();
        match (should_listen, self.listening.is_some()) {
            (true, false) => self.listening = Some(Listening::start(&self.sources)),
            (false, true) => self.listening = None,
            _ => {}
        }
    }

    fn retime(&mut self) {
        let cadence = match self.following.timeline && self.playing() {
            true => Period::Second,
            false => self.following.cadence,
        };
        if self
            .cadence
            .as_ref()
            .is_some_and(|(current, _)| *current == cadence)
        {
            return;
        }
        self.cadence = Some((cadence, self.sources.feeds.wall.need(cadence)));
    }
}
