use rikka_sync::Mutex;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Instant;

use futures_channel::mpsc::{UnboundedReceiver, unbounded};
use futures_util::{Stream, StreamExt};
use zbus::blocking::Connection;
use zbus::blocking::connection::Builder;
use zbus::fdo::{RequestNameFlags, RequestNameReply};
use zbus::object_server::SignalEmitter;
use zbus::zvariant::{OwnedValue, Value};

use crate::error::{Error, Result};
use crate::names::{PATH, PLAYER, PROPERTIES, bus_name};
use crate::publish::playback::{Command, Playback, PlayerState};
use crate::publish::player::{self, Player, Shared, micros};
use crate::publish::root::Root;

#[derive(Clone)]
pub struct Publisher {
    connection: Connection,
    shared: Shared,
    name: Arc<str>,
}

#[must_use]
#[derive(Debug)]
pub struct PublisherBuilder {
    program: String,
    identity: Option<String>,
    desktop_entry: Option<String>,
    raises: bool,
    quits: bool,
}

#[derive(Debug)]
pub struct Commands(UnboundedReceiver<Command>);

impl Publisher {
    pub fn builder(program: impl Into<String>) -> PublisherBuilder {
        PublisherBuilder {
            program: program.into(),
            identity: None,
            desktop_entry: None,
            raises: false,
            quits: false,
        }
    }

    #[must_use]
    pub fn bus_name(&self) -> &str {
        &self.name
    }

    pub fn show(&self, playback: Playback) {
        let now = Instant::now();
        let mut state = self.shared.lock();
        if state.playback == playback {
            return;
        }
        let jumped = state.jumped(&playback, now);
        let changed = changed(&state.playback, &playback);
        state.playback = playback;
        state.since = now;
        let values: HashMap<&str, OwnedValue> = changed
            .into_iter()
            .filter_map(|property| Some((property, value(&state, property)?)))
            .collect();
        let position = micros(state.playback.position);
        drop(state);

        if !values.is_empty() {
            let emitted = self.connection.emit_signal(
                None::<&str>,
                PATH,
                PROPERTIES,
                "PropertiesChanged",
                &(PLAYER, values, Vec::<&str>::new()),
            );
            if let Err(error) = emitted {
                tracing::debug!("failed to emit playback change for {}: {error}", self.name);
            }
        }
        if jumped {
            let emitted = SignalEmitter::new(self.connection.inner(), PATH)
                .and_then(|emitter| zbus::block_on(Player::seeked(&emitter, position)));
            if let Err(error) = emitted {
                tracing::debug!("failed to emit Seeked for {}: {error}", self.name);
            }
        }
    }
}

impl std::fmt::Debug for Publisher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Publisher")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

impl PublisherBuilder {
    pub fn identity(mut self, identity: impl Into<String>) -> Self {
        self.identity = Some(identity.into());
        self
    }

    pub fn desktop_entry(mut self, entry: impl Into<String>) -> Self {
        self.desktop_entry = Some(entry.into());
        self
    }

    pub fn raises(mut self) -> Self {
        self.raises = true;
        self
    }

    pub fn quits(mut self) -> Self {
        self.quits = true;
        self
    }

    pub fn serve(self) -> Result<(Publisher, Commands)> {
        let (commands, receiver) = unbounded();
        let shared: Shared = Arc::new(Mutex::new(PlayerState::new()));
        let root = Root {
            identity: self.identity.unwrap_or_else(|| self.program.clone()),
            desktop_entry: self.desktop_entry,
            raises: self.raises,
            quits: self.quits,
            commands: commands.clone(),
        };
        let player = Player {
            shared: Arc::clone(&shared),
            commands,
        };
        let connection = Builder::session()
            .and_then(|builder| builder.serve_at(PATH, root))
            .and_then(|builder| builder.serve_at(PATH, player))
            .and_then(Builder::build)
            .map_err(|error| Error::Unreachable(error.to_string()))?;
        let name = claim(&connection, &self.program)?;
        Ok((
            Publisher {
                connection,
                shared,
                name: name.into(),
            },
            Commands(receiver),
        ))
    }
}

impl Stream for Commands {
    type Item = Command;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Command>> {
        self.0.poll_next_unpin(cx)
    }
}

fn claim(connection: &Connection, program: &str) -> Result<String> {
    let usual = bus_name(program);
    let instance = format!("{usual}.instance{}", std::process::id());
    for name in [usual, instance] {
        let reply = connection
            .request_name_with_flags(name.as_str(), RequestNameFlags::DoNotQueue.into())
            .map_err(|error| Error::Refused(error.to_string()))?;
        if reply == RequestNameReply::PrimaryOwner {
            return Ok(name);
        }
    }
    Err(Error::Refused(format!("{program} is already playing here")))
}

fn changed(previous: &Playback, next: &Playback) -> Vec<&'static str> {
    let mut changed = Vec::new();
    let mut note = |differs: bool, property: &'static str| {
        if differs {
            changed.push(property);
        }
    };
    note(previous.status != next.status, "PlaybackStatus");
    note(previous.track != next.track, "Metadata");
    note(previous.volume != next.volume, "Volume");
    note(previous.rate != next.rate, "Rate");
    note(previous.shuffle != next.shuffle, "Shuffle");
    note(previous.repeat != next.repeat, "LoopStatus");
    let (was, is) = (previous.abilities, next.abilities);
    note(was.next != is.next, "CanGoNext");
    note(was.previous != is.previous, "CanGoPrevious");
    note(was.play != is.play, "CanPlay");
    note(was.pause != is.pause, "CanPause");
    note(was.seek != is.seek, "CanSeek");
    note(was.control != is.control, "CanControl");
    changed
}

fn value(state: &PlayerState, property: &str) -> Option<OwnedValue> {
    let playback = &state.playback;
    let value = match property {
        "PlaybackStatus" => Value::from(playback.status.name()),
        "Metadata" => Value::from(player::owned_metadata(state)),
        "Volume" => Value::from(playback.volume),
        "Rate" => Value::from(playback.rate),
        "Shuffle" => Value::from(playback.shuffle.unwrap_or(false)),
        "LoopStatus" => Value::from(playback.repeat.unwrap_or_default().name()),
        "CanGoNext" => Value::from(playback.abilities.next),
        "CanGoPrevious" => Value::from(playback.abilities.previous),
        "CanPlay" => Value::from(playback.abilities.play),
        "CanPause" => Value::from(playback.abilities.pause),
        "CanSeek" => Value::from(playback.abilities.seek),
        "CanControl" => Value::from(playback.abilities.control),
        _ => return None,
    };
    OwnedValue::try_from(value).ok()
}
