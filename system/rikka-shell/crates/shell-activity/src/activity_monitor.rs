use std::convert::Infallible;
use std::pin::Pin;

use futures_util::{Stream, StreamExt};
use rikka_battery::Battery;
use rikka_bluetooth::Bluetooth;
use rikka_mpris::Players;
use rikka_network::Network;
use rikka_power::Power;
use rikka_power_profiles::PowerProfiles;
use rikka_privacy::Privacy;
use rikka_sound::Sound;
use rikka_worker::{Observable, Stop, Worker};
use tokio::sync::{broadcast, mpsc};

use crate::activity::Activity;
use crate::activity_event::ActivityEvent;
use crate::tracker::ActivityTracker;

const NAME: &str = "rikka-activity";
const BACKLOG: usize = 64;

#[derive(Clone, Debug)]
pub struct Sources {
    pub sound: Sound,
    pub battery: Battery,
    pub bluetooth: Bluetooth,
    pub power: Power,
    pub network: Network,
    pub profiles: PowerProfiles,
    pub media: Players,
    pub privacy: Privacy,
}

enum Update {
    Sound(rikka_sound::Snapshot),
    Battery(rikka_battery::State),
    Bluetooth(rikka_bluetooth::State),
    Power(rikka_power::State),
    Network(rikka_network::State),
    Profiles(rikka_power_profiles::State),
    Media(rikka_mpris::State),
    Privacy(rikka_privacy::State),
    Shutdown,
}

type UpdateStream = Pin<Box<dyn Stream<Item = Update> + Send>>;

struct Shutdown(mpsc::UnboundedSender<()>);

impl Stop for Shutdown {
    fn stop(&self) {
        if self.0.send(()).is_err() {
            tracing::debug!("activity monitor already stopped");
        }
    }
}

pub struct ActivityMonitor {
    activity_events: broadcast::Sender<ActivityEvent>,
    activities: Observable<Vec<Activity>>,
    worker: Option<Worker<Shutdown>>,
}

#[derive(Clone, Debug)]
pub struct Activities(Observable<Vec<Activity>>);

impl ActivityMonitor {
    #[must_use]
    pub fn start(sources: Sources) -> ActivityMonitor {
        let (activity_events, _) = broadcast::channel(BACKLOG);
        let activities = Observable::new(Vec::new());
        let (shutdown, shutdown_rx) = mpsc::unbounded_channel();
        let (events_tx, activities_tx) = (activity_events.clone(), activities.clone());
        let worker = Worker::spawn(NAME, Shutdown(shutdown), move || {
            Ok(move || {
                rikka_worker::block_on(monitor(sources, shutdown_rx, &events_tx, &activities_tx));
            })
        })
        .inspect_err(|error: &rikka_worker::Error<Infallible>| {
            tracing::warn!("cannot start activity monitor: {error}");
        })
        .ok();
        ActivityMonitor {
            activity_events,
            activities,
            worker,
        }
    }

    pub fn activity_events(&self) -> impl Stream<Item = ActivityEvent> + Send + 'static {
        let receiver = self.activity_events.subscribe();
        futures_util::stream::unfold(receiver, |mut receiver| async move {
            loop {
                match receiver.recv().await {
                    Ok(activity_event) => return Some((activity_event, receiver)),
                    Err(broadcast::error::RecvError::Lagged(_)) => {}
                    Err(broadcast::error::RecvError::Closed) => return None,
                }
            }
        })
    }

    #[must_use]
    pub fn activities(&self) -> Activities {
        Activities(self.activities.clone())
    }
}

impl std::fmt::Debug for ActivityMonitor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ActivityMonitor")
            .field("running", &self.worker.is_some())
            .finish_non_exhaustive()
    }
}

impl Activities {
    #[must_use]
    pub fn state(&self) -> Vec<Activity> {
        self.0.get()
    }

    pub fn changes(&self) -> impl Stream<Item = Vec<Activity>> + Send + 'static {
        self.0.changes()
    }
}

async fn monitor(
    sources: Sources,
    mut shutdown_rx: mpsc::UnboundedReceiver<()>,
    activity_events: &broadcast::Sender<ActivityEvent>,
    activities: &Observable<Vec<Activity>>,
) {
    let streams: Vec<UpdateStream> = vec![
        sources.sound.changes().map(Update::Sound).boxed(),
        sources.battery.changes().map(Update::Battery).boxed(),
        sources.bluetooth.changes().map(Update::Bluetooth).boxed(),
        sources.power.changes().map(Update::Power).boxed(),
        sources.network.changes().map(Update::Network).boxed(),
        sources.profiles.changes().map(Update::Profiles).boxed(),
        sources.media.changes().map(Update::Media).boxed(),
        sources.privacy.changes().map(Update::Privacy).boxed(),
        futures_util::stream::once(async move {
            shutdown_rx.recv().await;
            Update::Shutdown
        })
        .boxed(),
    ];
    let mut updates = futures_util::stream::select_all(streams);

    let mut tracker = ActivityTracker::default();
    tracker.sound(&sources.sound.state());
    tracker.battery(sources.battery.state());
    tracker.bluetooth(&sources.bluetooth.state());
    tracker.power(&sources.power.state());
    tracker.network(&sources.network.state());
    tracker.profiles(&sources.profiles.state());
    tracker.media(sources.media.state());
    tracker.privacy(sources.privacy.state());
    activities.update(tracker.activities());

    while let Some(update) = updates.next().await {
        let (events, ongoing) = match update {
            Update::Shutdown => return,
            Update::Sound(graph) => (tracker.sound(&graph), false),
            Update::Bluetooth(state) => (tracker.bluetooth(&state), false),
            Update::Power(state) => (tracker.power(&state), false),
            Update::Network(state) => (tracker.network(&state), false),
            Update::Profiles(state) => (tracker.profiles(&state), false),
            Update::Battery(state) => (tracker.battery(state), true),
            Update::Media(state) => (tracker.media(state), true),
            Update::Privacy(state) => (tracker.privacy(state), true),
        };
        if activity_events.receiver_count() > 0 {
            for activity_event in events {
                if activity_events.send(activity_event).is_err() {
                    break;
                }
            }
        }
        if ongoing {
            activities.update(tracker.activities());
        }
    }
}
