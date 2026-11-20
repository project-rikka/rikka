mod format;
mod live;
mod meta;

use std::cell::RefCell;
use std::collections::HashMap;
use std::convert::Infallible;
use std::rc::{Rc, Weak};
use std::time::Duration;

use futures_channel::oneshot;
use libspa::pod::Pod;
use libspa::utils::Direction;
use pipewire::keys;
use pipewire::properties::properties;
use pipewire::stream::{StreamFlags, StreamListener, StreamRc, StreamState};
use pipewire::types::ObjectType;
use rikka_capture::{Screen, Source};
use rikka_pipewire::{Connection, LoopWorker};
use rikka_privacy::{CAST_APP, SCREENCAST as NODE_NAME};

use crate::backend::cast::live::{CastStreamState, Placement};
use crate::error::{Error, Result};
use crate::protocol::{CastSource, SourceKind};

const THREAD: &str = "rikka-screencast";

const NODE_TIMEOUT: Duration = Duration::from_secs(10);

const REDRAW_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CursorMode {
    Hidden,
    Embedded,
    Metadata,
}

#[derive(Clone, Debug)]
pub(crate) struct CastStream {
    pub(crate) source: CastSource,
    pub(crate) node: u32,
    pub(crate) serial: u64,
    pub(crate) position: Option<(i32, i32)>,
    pub(crate) size: (i32, i32),
}

pub(crate) struct Cast {
    _worker: LoopWorker,
}

impl Cast {
    pub(crate) async fn start(
        sources: Vec<CastSource>,
        cursor: CursorMode,
        app: String,
    ) -> Result<(Cast, Vec<CastStream>)> {
        if sources.is_empty() {
            return Err(Error::NoSources);
        }
        let (ready_sender, ready) = oneshot::channel();
        let worker = rikka_pipewire::spawn(THREAD, move |stopped| {
            Ok::<_, Infallible>(move || run(sources, cursor, &app, stopped, ready_sender))
        })
        .map_err(|error| match error {
            rikka_worker::Error::Spawn(error) => Error::Thread(error),
            rikka_worker::Error::Startup(_) | rikka_worker::Error::Exited => Error::NoResponse,
        })?;

        let cast = Cast { _worker: worker };
        match ready.await {
            Ok(Ok(streams)) => Ok((cast, streams)),
            Ok(Err(error)) => Err(error),
            Err(_) => Err(Error::NoResponse),
        }
    }
}

impl std::fmt::Debug for Cast {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Cast").finish_non_exhaustive()
    }
}

struct Session {
    screen: Screen,
    streams: Vec<CastStreamState>,
    serials: HashMap<u32, u64>,
    ready_sender: Option<oneshot::Sender<Result<Vec<CastStream>>>>,
}

impl Session {
    fn report_ready(&mut self) {
        if self.ready_sender.is_none() {
            return;
        }
        for live in &mut self.streams {
            live.settle_serial(&self.serials);
        }
        let streams: Option<Vec<CastStream>> = self
            .streams
            .iter()
            .map(|live| {
                let (node, serial) = live.published()?;
                Some(CastStream {
                    source: live.source.clone(),
                    node,
                    serial,
                    position: live.place.position,
                    size: live.place.size,
                })
            })
            .collect();
        if let Some(streams) = streams
            && let Some(ready_sender) = self.ready_sender.take()
        {
            let _ = ready_sender.send(Ok(streams));
        }
    }

    fn fail(&mut self, error: Error) {
        if let Some(ready_sender) = self.ready_sender.take() {
            let _ = ready_sender.send(Err(error));
        }
    }

    fn handle_events(&mut self) -> bool {
        let Session {
            screen, streams, ..
        } = self;
        let mut alive = false;
        for live in streams.iter_mut() {
            alive |= live.handle_events(screen);
        }
        flush(screen);
        alive
    }
}

fn run(
    sources: Vec<CastSource>,
    cursor: CursorMode,
    app: &str,
    stopped: pipewire::channel::Receiver<()>,
    ready_sender: oneshot::Sender<Result<Vec<CastStream>>>,
) {
    let connection = match Connection::open() {
        Ok(connection) => connection,
        Err(error) => {
            let _ = ready_sender.send(Err(Error::Connect(error)));
            return;
        }
    };
    let main_loop = connection.main_loop();
    let core = connection.core();
    let registry = connection.registry();
    let screen = match Screen::connect() {
        Ok(screen) => screen,
        Err(error) => {
            let _ = ready_sender.send(Err(error.into()));
            return;
        }
    };

    let session = Rc::new(RefCell::new(Session {
        screen,
        streams: Vec::new(),
        serials: HashMap::new(),
        ready_sender: Some(ready_sender),
    }));

    if let Err(error) = create_streams(&session, core, &sources, cursor, app) {
        session.borrow_mut().fail(error);
        return;
    }

    let mut listeners = Vec::new();
    for index in 0..sources.len() {
        match add_listener(&session, index) {
            Some(listener) => listeners.push(listener),
            None => {
                session.borrow_mut().fail(Error::AddListener);
                return;
            }
        }
    }
    if let Err(error) = connect_streams(&session) {
        session.borrow_mut().fail(error);
        return;
    }

    let error_loop = main_loop.downgrade();
    let _core_listener = core
        .add_listener_local()
        .error(move |id, _, _, _| {
            if id == 0
                && let Some(main_loop) = error_loop.upgrade()
            {
                main_loop.quit();
            }
        })
        .register();

    let registry_session = Rc::downgrade(&session);
    let _registry_listener = registry
        .add_listener_local()
        .global(move |global| {
            if global.type_ != ObjectType::Node {
                return;
            }
            let Some(serial) = global
                .props
                .and_then(|props| props.get(*keys::OBJECT_SERIAL))
                .and_then(|serial| serial.parse().ok())
            else {
                return;
            };
            with_session(&registry_session, |session| {
                session.serials.insert(global.id, serial);
                session.report_ready();
            });
        })
        .register();

    let display_session = Rc::downgrade(&session);
    let display_loop = main_loop.downgrade();
    let watched = session
        .borrow()
        .screen
        .as_fd()
        .try_clone_to_owned()
        .map_err(Error::WatchDisplay);
    let watched = match watched {
        Ok(watched) => watched,
        Err(error) => {
            session.borrow_mut().fail(error);
            return;
        }
    };
    let _wayland =
        main_loop
            .loop_()
            .add_io(watched, libspa::support::system::IoFlags::IN, move |_| {
                let Some(session) = display_session.upgrade() else {
                    return;
                };
                let mut session = session.borrow_mut();
                let read = session.screen.dispatch(Some(std::time::Instant::now()));
                let sharing = read.is_ok() && session.handle_events();
                drop(session);
                if !sharing && let Some(main_loop) = display_loop.upgrade() {
                    main_loop.quit();
                }
            });

    let timeout_session = Rc::downgrade(&session);
    let deadline = main_loop.loop_().add_timer(move |_| {
        with_session(&timeout_session, |session| {
            session.fail(Error::NodeTimeout);
        });
    });
    arm_timer(&deadline, NODE_TIMEOUT, None);

    let redraw_session = Rc::downgrade(&session);
    let redraw_timer = main_loop.loop_().add_timer(move |_| {
        with_session(&redraw_session, |session| {
            let Session {
                screen, streams, ..
            } = session;
            for live in streams.iter_mut() {
                live.pump_frames(screen);
            }
            flush(screen);
        });
    });
    arm_timer(&redraw_timer, REDRAW_INTERVAL, Some(REDRAW_INTERVAL));

    connection.run_until_stopped(stopped);
    drop(listeners);
    session.borrow_mut().streams.clear();
}

fn flush(screen: &mut Screen) {
    if let Err(error) = screen.flush() {
        eprintln!("rikka-portal-cast: cannot flush Wayland connection: {error}");
    }
}

fn arm_timer(
    timer: &pipewire::loop_::TimerSource<'_>,
    value: Duration,
    interval: Option<Duration>,
) {
    if let Err(error) = timer.update_timer(Some(value), interval).into_result() {
        eprintln!("rikka-portal-cast: cannot arm timer: {error}");
    }
}

fn with_session(session: &Weak<RefCell<Session>>, act: impl FnOnce(&mut Session)) {
    let Some(session) = session.upgrade() else {
        return;
    };
    act(&mut session.borrow_mut());
}

fn try_with_session(session: &Weak<RefCell<Session>>, act: impl FnOnce(&mut Session)) {
    let Some(session) = session.upgrade() else {
        return;
    };
    if let Ok(mut session) = session.try_borrow_mut() {
        act(&mut session);
    }
}

fn create_streams(
    session: &Rc<RefCell<Session>>,
    core: &pipewire::core::CoreRc,
    sources: &[CastSource],
    cursor: CursorMode,
    app: &str,
) -> Result<()> {
    let mut session = session.borrow_mut();
    for source in sources {
        let (capture_source, mut place, description) = resolve_source(&mut session.screen, source)?;
        let props = properties! {
            *keys::MEDIA_CLASS => "Video/Source",
            *keys::MEDIA_TYPE => "Video",
            *keys::MEDIA_CATEGORY => "Capture",
            *keys::MEDIA_ROLE => "Screen",
            *keys::NODE_NAME => NODE_NAME,
            *keys::NODE_DESCRIPTION => description.as_str(),
            CAST_APP => app,
        };
        let stream = StreamRc::new(core.clone(), NODE_NAME, props).map_err(Error::CreateStream)?;

        let screen = &mut session.screen;
        let mut live = CastStreamState::open(
            screen,
            stream,
            source.clone(),
            place,
            &capture_source,
            cursor,
        )?;
        if place.size.0 == 0 || place.size.1 == 0 {
            let constraints = live.constraints();
            place.size = (constraints.width as i32, constraints.height as i32);
            live.place = place;
        }
        session.streams.push(live);
    }
    Ok(())
}

fn resolve_source(screen: &mut Screen, source: &CastSource) -> Result<(Source, Placement, String)> {
    match source.kind {
        SourceKind::Monitor => {
            let outputs = screen.outputs();
            let output = outputs
                .into_iter()
                .find(|output| output.name == source.id)
                .ok_or_else(|| Error::NoDisplay(source.id.clone()))?;
            let described = match output.description.is_empty() {
                true => output.name.clone(),
                false => output.description.clone(),
            };
            Ok((
                Source::Output(output.name.clone()),
                Placement {
                    position: Some((output.x, output.y)),
                    size: (output.logical_width as i32, output.logical_height as i32),
                },
                described,
            ))
        }
        SourceKind::Window => {
            let windows = screen.windows();
            let window = windows
                .into_iter()
                .find(|window| window.identifier == source.id)
                .ok_or_else(|| Error::NoWindow(source.id.clone()))?;
            let described = match window.title.is_empty() {
                true => window.app_id.clone(),
                false => window.title.clone(),
            };
            Ok((
                Source::Window(window.identifier.clone()),
                Placement {
                    position: None,
                    size: (0, 0),
                },
                described,
            ))
        }
    }
}

fn add_listener(session: &Rc<RefCell<Session>>, index: usize) -> Option<StreamListener<()>> {
    let stream = session.borrow().streams.get(index)?.stream().clone();

    let changing = Rc::downgrade(session);
    let asking = Rc::downgrade(session);
    let adding = Rc::downgrade(session);
    let removing = Rc::downgrade(session);
    let drawing = Rc::downgrade(session);

    stream
        .add_local_listener_with_user_data(())
        .state_changed(move |stream, (), _old, new| {
            if !matches!(new, StreamState::Paused | StreamState::Streaming) {
                return;
            }
            let node = stream.node_id();
            let streaming = new == StreamState::Streaming;
            with_session(&changing, |session| {
                let (streams, screen) = (&mut session.streams, &mut session.screen);
                if let Some(live) = streams.get_mut(index) {
                    live.found_node(node);
                    if streaming {
                        live.pump_frames(screen);
                        flush(screen);
                    }
                }
                session.report_ready();
            });
        })
        .param_changed(move |_stream, (), id, param| {
            with_session(&asking, |session| {
                let (streams, screen) = (&mut session.streams, &mut session.screen);
                if let Some(live) = streams.get_mut(index) {
                    live.param_changed(screen, id, param);
                }
            });
        })
        .add_buffer(move |_stream, (), buffer| {
            with_session(&adding, |session| {
                let (streams, screen) = (&mut session.streams, &mut session.screen);
                if let Some(live) = streams.get_mut(index) {
                    live.add_buffer(screen, buffer);
                    live.pump_frames(screen);
                }
                flush(screen);
            });
        })
        .remove_buffer(move |_stream, (), buffer| {
            with_session(&removing, |session| {
                if let Some(live) = session.streams.get_mut(index) {
                    live.remove_buffer(buffer);
                }
            });
        })
        .process(move |_stream, ()| {
            try_with_session(&drawing, |session| {
                let (streams, screen) = (&mut session.streams, &mut session.screen);
                if let Some(live) = streams.get_mut(index) {
                    live.pump_frames(screen);
                }
                flush(screen);
            });
        })
        .register()
        .ok()
}

fn connect_streams(session: &Rc<RefCell<Session>>) -> Result<()> {
    let mut session = session.borrow_mut();
    for live in &mut session.streams {
        let offers = format::offers(live.constraints());
        let mut pods: Vec<&Pod> = Vec::with_capacity(offers.len());
        for offer in &offers {
            if let Some(pod) = Pod::from_bytes(offer) {
                pods.push(pod);
            }
        }
        if pods.is_empty() {
            return Err(Error::NoFormat);
        }
        live.stream()
            .connect(
                Direction::Output,
                None,
                StreamFlags::DRIVER | StreamFlags::ALLOC_BUFFERS,
                &mut pods,
            )
            .map_err(Error::ConnectStream)?;
    }
    Ok(())
}
