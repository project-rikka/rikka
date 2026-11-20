use std::collections::{BTreeMap, HashMap};
use std::time::{Duration, Instant};

use zbus::blocking::Connection;
use zbus::blocking::connection::Builder;
use zbus::blocking::fdo::DBusProxy;
use zbus::zvariant::{ObjectPath, OwnedValue, Value};

use crate::metadata;
use crate::names::{PATH, PLAYER, PREFIX, PROXIES, ROOT};
use crate::track::{Abilities, Repeat, Status};
use crate::watch::choice;
use crate::watch::listen::{self, Changed, Changes};
use crate::watch::player::{Control, Player, PlayerId};
use crate::watch::state::State;
use rikka_bus::{EventSink, Handler, Notifier, Properties, StartError, Watch};

const PATIENCE: Duration = Duration::from_secs(2);
const DRIFT: Duration = Duration::from_millis(500);

const EVERYTHING: Changed = Changed {
    root: true,
    player: true,
};

pub(crate) struct Request {
    pub(crate) player: PlayerId,
    pub(crate) control: Control,
}

pub(crate) struct Mpris {
    connection: Connection,
    owners: BTreeMap<String, String>,
    players: BTreeMap<String, Player>,
    _signals: Option<Watch>,
}

impl Handler for Mpris {
    type Config = ();
    type Request = Request;
    type Notice = Changes;
    type Event = State;
    type State = State;

    const THREAD_NAME: &'static str = "rikka-mpris";
    const UNAVAILABLE: &'static str = "this session shows no media players";

    fn open(
        (): (),
        notifier: Notifier<Mpris>,
        report: &EventSink<State>,
    ) -> Result<Mpris, StartError> {
        let connection = Builder::session()?.method_timeout(PATIENCE).build()?;
        let signals = listen::listen(&connection, notifier)
            .inspect_err(|error| tracing::warn!("failed to watch MPRIS players: {error}"))
            .ok();
        let mut mpris = Mpris {
            connection,
            owners: BTreeMap::new(),
            players: BTreeMap::new(),
            _signals: signals,
        };
        let bus = DBusProxy::new(&mpris.connection)?;
        for name in bus.list_names()? {
            let name = name.to_string();
            if !watched(&name) {
                continue;
            }
            if let Ok(owner) = bus.get_name_owner(name.as_str().try_into()?) {
                mpris.owners.insert(name.clone(), owner.to_string());
                mpris.reread(&name, EVERYTHING);
            }
        }
        report.emit(mpris.state());
        Ok(mpris)
    }

    fn handle_request(&mut self, request: Request, _report: &EventSink<State>) {
        let name = request.player.as_str();
        if let Err(error) = self.control(name, &request.control) {
            tracing::debug!("{name} rejected {:?}: {error}", request.control);
        }
    }

    fn handle_notice(&mut self, notice: Changes, report: &EventSink<State>) {
        for (name, owner) in notice.owners {
            if !watched(&name) {
                continue;
            }
            match owner.is_empty() {
                true => {
                    self.owners.remove(&name);
                    self.players.remove(&name);
                }
                false => {
                    self.owners.insert(name.clone(), owner);
                    self.reread(&name, EVERYTHING);
                }
            }
        }
        let changed: Vec<(String, Changed)> = self
            .owners
            .iter()
            .filter_map(|(name, owner)| Some((name.clone(), *notice.changed.get(owner)?)))
            .collect();
        for (name, changed) in changed {
            self.reread(&name, changed);
        }
        let now = Instant::now();
        for (sender, at) in notice.seeked {
            for (name, owner) in &self.owners {
                if *owner != sender {
                    continue;
                }
                if let Some(player) = self.players.get_mut(name) {
                    player.position = micros(at);
                    player.read_at = now;
                }
            }
        }
        report.emit(self.state());
    }
}

impl Mpris {
    fn state(&self) -> State {
        State {
            active: choice::active(self.players.values()),
            players: self.players.values().cloned().collect(),
        }
    }

    fn reread(&mut self, name: &str, changed: Changed) {
        let known = self.players.get(name);
        let identity = match (changed.root, known) {
            (false, Some(known)) => Identity::new(known),
            _ => self.identity(name),
        };
        let read = match (changed.player, known) {
            (false, Some(known)) => Some(identity.apply_to(known.clone())),
            _ => self.playing(name, identity),
        };
        let Some(mut read) = read else {
            self.players.remove(name);
            return;
        };
        let now = read.read_at;
        choice::activity(known, &mut read, now);
        if let Some(known) = known {
            steady(known, &mut read);
        }
        self.players.insert(name.to_owned(), read);
    }

    fn properties(&self, name: &str, interface: &str) -> Option<Properties> {
        Properties::read(&self.connection, name, PATH, interface)
            .inspect_err(|error| tracing::debug!("failed to read {interface} from {name}: {error}"))
            .ok()
    }

    fn identity(&self, name: &str) -> Identity {
        let root = self.properties(name, ROOT).unwrap_or_default();
        Identity {
            identity: text(&root, "Identity").unwrap_or_else(|| fallback_identity(name)),
            desktop_entry: text(&root, "DesktopEntry"),
            raises: root.boolean("CanRaise").unwrap_or(false),
        }
    }

    fn playing(&self, name: &str, identity: Identity) -> Option<Player> {
        let player = self.properties(name, PLAYER)?;
        Some(Player {
            id: PlayerId(name.to_owned()),
            identity: identity.identity,
            desktop_entry: identity.desktop_entry,
            status: Status::named(player.string_ref("PlaybackStatus").unwrap_or_default()),
            track: player
                .value::<HashMap<String, OwnedValue>>("Metadata")
                .and_then(|metadata| metadata::decode(&metadata.into())),
            position: player.int64("Position").map_or(Duration::ZERO, micros),
            read_at: Instant::now(),
            rate: player
                .double("Rate")
                .filter(|rate| *rate > 0.0)
                .unwrap_or(1.0),
            volume: player.double("Volume"),
            shuffle: player.boolean("Shuffle"),
            repeat: player.string_ref("LoopStatus").and_then(Repeat::named),
            abilities: Abilities {
                play: player.boolean("CanPlay").unwrap_or(false),
                pause: player.boolean("CanPause").unwrap_or(false),
                next: player.boolean("CanGoNext").unwrap_or(false),
                previous: player.boolean("CanGoPrevious").unwrap_or(false),
                seek: player.boolean("CanSeek").unwrap_or(false),
                control: player.boolean("CanControl").unwrap_or(false),
            },
            raises: identity.raises,
            active_at: None,
        })
    }

    fn control(&self, name: &str, control: &Control) -> zbus::Result<()> {
        let call = |interface: &str, method: &str| {
            self.connection
                .call_method(Some(name), PATH, Some(interface), method, &())
                .map(drop)
        };
        let set = |property: &str, value: Value<'_>| {
            self.connection
                .call_method(
                    Some(name),
                    PATH,
                    Some(crate::names::PROPERTIES),
                    "Set",
                    &(PLAYER, property, value),
                )
                .map(drop)
        };
        match control {
            Control::Play => call(PLAYER, "Play"),
            Control::Pause => call(PLAYER, "Pause"),
            Control::PlayPause => call(PLAYER, "PlayPause"),
            Control::Stop => call(PLAYER, "Stop"),
            Control::Next => call(PLAYER, "Next"),
            Control::Previous => call(PLAYER, "Previous"),
            Control::Raise => call(ROOT, "Raise"),
            Control::SeekBy { forward, by } => {
                let micros = i64::try_from(by.as_micros()).unwrap_or(i64::MAX);
                let offset = if *forward { micros } else { -micros };
                self.connection
                    .call_method(Some(name), PATH, Some(PLAYER), "Seek", &offset)
                    .map(drop)
            }
            Control::SeekTo(position) => {
                let Some(track) = self
                    .players
                    .get(name)
                    .and_then(|player| player.track.as_ref())
                else {
                    return Ok(());
                };
                let track = ObjectPath::try_from(track.id.as_str())?;
                let micros = i64::try_from(position.as_micros()).unwrap_or(i64::MAX);
                self.connection
                    .call_method(
                        Some(name),
                        PATH,
                        Some(PLAYER),
                        "SetPosition",
                        &(track, micros),
                    )
                    .map(drop)
            }
            Control::Volume(volume) => set("Volume", Value::from(volume.max(0.0))),
            Control::Shuffle(shuffle) => set("Shuffle", Value::from(*shuffle)),
            Control::Repeat(repeat) => set("LoopStatus", Value::from(repeat.name())),
        }
    }
}

fn watched(name: &str) -> bool {
    name.strip_prefix(PREFIX).is_some_and(|rest| {
        let program = rest.split('.').next().unwrap_or_default();
        !PROXIES.contains(&program)
    })
}

fn fallback_identity(name: &str) -> String {
    let rest = name.strip_prefix(PREFIX).unwrap_or(name);
    rest.split('.').next().unwrap_or(rest).to_owned()
}

fn text(properties: &Properties, key: &str) -> Option<String> {
    properties
        .string_ref(key)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}

fn micros(micros: i64) -> Duration {
    Duration::from_micros(micros.max(0).unsigned_abs())
}

fn steady(previous: &Player, read: &mut Player) {
    let same = previous.status == read.status
        && previous.rate == read.rate
        && previous.track.as_ref().map(|track| &track.id)
            == read.track.as_ref().map(|track| &track.id);
    if same && previous.position_at(read.read_at).abs_diff(read.position) < DRIFT {
        read.position = previous.position;
        read.read_at = previous.read_at;
    }
}

struct Identity {
    identity: String,
    desktop_entry: Option<String>,
    raises: bool,
}

impl Identity {
    fn new(player: &Player) -> Identity {
        Identity {
            identity: player.identity.clone(),
            desktop_entry: player.desktop_entry.clone(),
            raises: player.raises,
        }
    }

    fn apply_to(self, player: Player) -> Player {
        Player {
            identity: self.identity,
            desktop_entry: self.desktop_entry,
            raises: self.raises,
            ..player
        }
    }
}
