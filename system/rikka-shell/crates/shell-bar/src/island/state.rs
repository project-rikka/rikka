use std::rc::Rc;
use std::time::Duration;

use rikka_mpris::{Control, Player};
use rikka_session::{toplevels, workspaces};
use rikka_sound::Spectrum;
use rikka_types::{Point, PointerButton};
use rikka_ui::{State, after_keyed, skip_view, stream};
use rikka_weather::TemperatureUnit;
use shell_activity::{Activity, ActivityEvent};
use shell_feeds::{Feeds, ScreenRecording};
use shell_kit::{Need, Period, Placement};

use crate::island::clock::Clock;
use crate::island::forecast::Forecast;
use crate::island::live::{self, Live};
use crate::island::notice;
use crate::tasks::{Aim, MenuRequest, Opened, Tasks, Touch};

const OPEN_AFTER: Duration = Duration::from_millis(120);
const PREVIEW_AFTER: Duration = Duration::from_millis(400);
const CLOSE_AFTER: Duration = Duration::from_millis(260);
const PEEK_FOR: Duration = Duration::from_millis(2600);

const INTENT: &str = "island.intent";
const PEEKING: &str = "island.peek";

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Target {
    Live,
    Task(Aim),
}

impl Target {
    fn delay(&self) -> Duration {
        match self {
            Target::Live => OPEN_AFTER,
            Target::Task(_) => PREVIEW_AFTER,
        }
    }
}

#[derive(Debug, State)]
pub(crate) struct Island {
    feeds: Feeds,
    recording: ScreenRecording,
    tasks: Tasks,
    menu: Opened,
    width: f32,
    hovering: bool,
    aim: Option<Target>,
    showing: Option<Target>,
    peek: Option<ActivityEvent>,
    activities: Vec<Activity>,
    listening: Option<Rc<Spectrum>>,
    #[untracked]
    minutes: Need,
    #[untracked]
    seconds: Option<Need>,
    forecast: Option<Forecast>,
    unit: TemperatureUnit,
}

impl Island {
    #[must_use]
    pub(crate) fn new(
        feeds: Feeds,
        recording: ScreenRecording,
        tasks: Tasks,
        menu: Opened,
        width: f32,
    ) -> Island {
        let unit = feeds.scale.get();
        let minutes = feeds.wall.need(Period::Minute);
        Island {
            feeds,
            recording,
            tasks,
            menu,
            width,
            hovering: false,
            aim: None,
            showing: None,
            peek: None,
            activities: Vec::new(),
            listening: None,
            minutes,
            seconds: None,
            forecast: None,
            unit,
        }
    }

    pub(crate) fn start(&mut self) {
        let services = self.feeds.services.clone();
        let activities = services.activities();
        self.event(activities.state());
        stream(activities.changes(), Island::event);
        stream(services.activity_events(), Island::on_activity_event);
        stream(self.recording.changes(), |island: &mut Island, _: bool| {
            island.pace();
        });
        let weather = services.weather();
        self.forecast = Forecast::from_state(&weather.state());
        stream(weather.changes(), Island::forecasted);
        stream(self.feeds.scale.changes(), |island: &mut Island, unit| {
            island.unit = unit;
        });
        self.pace();
        self.follow_tasks();
    }

    fn forecasted(&mut self, state: rikka_weather::State) {
        let forecast = Forecast::from_state(&state);
        if forecast == self.forecast {
            skip_view();
            return;
        }
        self.forecast = forecast;
    }

    fn follow_tasks(&mut self) {
        let seam = self.tasks.seam().clone();
        stream(
            toplevels::states(&seam),
            |island: &mut Island, windows: toplevels::State| {
                let changed = island.tasks.windows_changed(windows);
                island.retasked(changed);
            },
        );
        stream(
            workspaces::states(&seam),
            |island: &mut Island, spaces: workspaces::State| {
                let changed = island.tasks.spaces_changed(spaces);
                island.retasked(changed);
            },
        );
        let apps = self.tasks.apps().clone();
        stream(apps.installed().changes(), |island: &mut Island, ()| {
            let changed = island.tasks.settle();
            island.retasked(changed);
        });
        stream(apps.icons().measured(), |island: &mut Island, ()| {
            let changed = island.tasks.settle();
            island.retasked(changed);
        });
    }

    pub(crate) fn moved(&mut self, placement: &Placement) {
        let resized = self.width != placement.size.width;
        self.width = placement.size.width;
        let changed = self.tasks.moved(placement.name.clone());
        self.retasked(changed || resized);
    }

    fn retasked(&mut self, changed: bool) {
        let lost = |target: &Option<Target>, tasks: &Tasks| match target {
            Some(Target::Task(aim)) => tasks.group(aim.space, &aim.key).is_none(),
            Some(Target::Live) | None => false,
        };
        if lost(&self.showing, &self.tasks) {
            self.showing = None;
        }
        if lost(&self.aim, &self.tasks) {
            self.aim = None;
        }
        if !changed && self.previewing().is_none() {
            skip_view();
        }
    }

    fn pace(&mut self) {
        let counting = self.counts_seconds();
        if counting != self.seconds.is_some() {
            self.seconds = counting.then(|| self.feeds.wall.need(Period::Second));
        }
    }

    fn counts_seconds(&self) -> bool {
        self.recording.since().is_some()
            || (self.is_open() && self.player().is_some_and(Player::is_playing))
    }

    pub(crate) fn hover(&mut self, over: bool) {
        self.hovering = over;
        let wait = match (over, &self.aim) {
            (true, Some(target)) => target.delay(),
            (true, None) => OPEN_AFTER,
            (false, _) => CLOSE_AFTER,
        };
        after_keyed(INTENT, wait, Island::settle);
        skip_view();
    }

    pub(crate) fn aim_live(&mut self, over: bool) {
        match over {
            true => self.aimed(Target::Live),
            false => skip_view(),
        }
    }

    pub(crate) fn hover_face(&mut self, over: bool) {
        match over && self.tasks.shown().is_empty() {
            true => self.aimed(Target::Live),
            false => skip_view(),
        }
    }

    fn aimed(&mut self, target: Target) {
        if self.aim.as_ref() == Some(&target) {
            skip_view();
            return;
        }
        self.aim = Some(target.clone());
        match self.showing {
            Some(_) => {
                self.showing = Some(target);
                self.pace();
            }
            None => {
                after_keyed(INTENT, target.delay(), Island::settle);
                skip_view();
            }
        }
    }

    fn settle(&mut self) {
        let showing = match self.hovering {
            true => self.aim.clone(),
            false => None,
        };
        if self.showing == showing {
            skip_view();
            return;
        }
        self.showing = showing;
        if self.showing.is_some() {
            self.peek = None;
        }
        self.pace();
    }

    fn dismiss(&mut self) {
        self.aim = None;
        self.showing = None;
        self.pace();
    }

    pub(crate) fn touched(&mut self, touch: Touch) {
        match touch {
            Touch::Aimed(aim) => self.aimed(Target::Task(aim)),
            Touch::Chose {
                space,
                key,
                button,
                at,
            } => self.chose(space, &key, button, at),
        }
    }

    fn chose(&mut self, space: usize, key: &str, button: PointerButton, at: Point) {
        match button {
            PointerButton::Primary => self.tasks.press(space, key),
            PointerButton::Middle => self.tasks.launch(key),
            PointerButton::Secondary => {
                self.unfold(space, key, at);
                return self.dismiss();
            }
            PointerButton::Back | PointerButton::Forward | PointerButton::Other(_) => {}
        }
        skip_view();
    }

    fn unfold(&self, space: usize, key: &str, at: Point) {
        let Some((group, tile)) = self.tasks.group(space, key) else {
            return;
        };
        let request = MenuRequest::new(at, space, group, tile);
        self.menu.open(self.tasks.output(), request);
    }

    pub(crate) fn focus_window(&mut self, id: &str) {
        self.tasks.focus(id);
        self.dismiss();
    }

    pub(crate) fn close_window(&mut self, id: &str) {
        self.tasks.close(id);
        skip_view();
    }

    fn event(&mut self, activities: Vec<Activity>) {
        let playing = live::player(&activities).is_some_and(Player::is_playing);
        if playing != self.listening.is_some() {
            self.listening = playing
                .then(|| self.feeds.audio_spectrum.listen())
                .flatten();
        }
        self.activities = activities;
        self.pace();
    }

    fn on_activity_event(&mut self, activity_event: ActivityEvent) {
        let hidden = self.is_open() && matches!(activity_event, ActivityEvent::Track { .. });
        if hidden || !notice::shows_notice(&activity_event) {
            skip_view();
            return;
        }
        self.peek = Some(activity_event);
        after_keyed(PEEKING, PEEK_FOR, |island: &mut Island| island.peek = None);
    }

    pub(crate) fn control(&mut self, control: Control) {
        if let Some(player) = self.player() {
            self.feeds.services.media().control(&player.id, control);
        }
        skip_view();
    }

    pub(crate) fn seek(&mut self, fraction: f32) {
        let Some(length) = self
            .player()
            .and_then(|player| player.track.as_ref())
            .and_then(|track| track.length)
        else {
            skip_view();
            return;
        };
        self.control(Control::SeekTo(length.mul_f32(fraction.clamp(0.0, 1.0))));
    }

    #[must_use]
    pub(crate) fn tasks(&self) -> &Tasks {
        &self.tasks
    }

    #[must_use]
    pub(crate) fn width(&self) -> f32 {
        self.width
    }

    #[must_use]
    pub(crate) fn is_open(&self) -> bool {
        self.showing == Some(Target::Live) && self.offers_controls()
    }

    #[must_use]
    pub(crate) fn previewing(&self) -> Option<&Aim> {
        match &self.showing {
            Some(Target::Task(aim)) => Some(aim),
            Some(Target::Live) | None => None,
        }
    }

    fn offers_controls(&self) -> bool {
        self.player().is_some() || self.recording.since().is_some()
    }

    #[must_use]
    pub(crate) fn peek(&self) -> Option<&ActivityEvent> {
        self.peek.as_ref()
    }

    #[must_use]
    pub(super) fn live_activities(&self) -> Vec<Live> {
        live::ranked(&self.activities, self.recording.since())
    }

    #[must_use]
    pub(crate) fn player(&self) -> Option<&Player> {
        live::player(&self.activities)
    }

    #[must_use]
    pub(crate) fn clock(&self) -> Clock {
        Clock::new(self.feeds.wall.now(Period::Minute))
    }

    #[must_use]
    pub(super) fn forecast(&self) -> Option<&Forecast> {
        self.forecast.as_ref()
    }

    #[must_use]
    pub(crate) fn unit(&self) -> TemperatureUnit {
        self.unit
    }

    #[must_use]
    pub(crate) fn elapsed(&self) -> Duration {
        self.feeds.wall.follow(Period::Second);
        let since = self.recording.since();
        Duration::from_secs(since.map_or(0, |since| since.elapsed().as_secs()))
    }

    #[must_use]
    pub(crate) fn position(&self, player: &Player) -> Duration {
        self.feeds.wall.follow(Period::Second);
        player.position_now()
    }

    #[must_use]
    pub(crate) fn spectrum(&self) -> Option<Spectrum> {
        self.listening.as_deref().cloned()
    }
}
