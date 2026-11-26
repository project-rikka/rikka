use std::{
    io,
    sync::{
        Arc,
        mpsc::{self, TryRecvError},
    },
    thread,
    time::Duration,
};

use crossterm::event::{self, Event as TerminalEvent};
use installer_core::Restored;
use installer_sys::{Command, Event, Host, install, read_restore};

use crate::{
    app::{App, Outcome},
    paint,
};

const TICK: Duration = Duration::from_millis(80);

pub fn run(app: &mut App, host: Arc<dyn Host>) -> io::Result<Outcome> {
    let mut terminal = ratatui::try_init().map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("no terminal available ({error}); run from a tty"),
        )
    })?;
    let outcome = drive(&mut terminal, app, host);
    ratatui::restore();
    outcome
}

fn drive(
    terminal: &mut ratatui::DefaultTerminal,
    app: &mut App,
    host: Arc<dyn Host>,
) -> io::Result<Outcome> {
    let (sender, receiver) = mpsc::channel::<Event>();
    let mut sender = Some(sender);
    let (read_sender, read_receiver) = mpsc::channel::<Result<Restored, String>>();

    let mut keymap = "us";

    loop {
        terminal.draw(|frame| paint::draw(frame, app))?;

        if let Some(outcome) = app.outcome() {
            return Ok(outcome);
        }

        if app.take_start_request()
            && let Some(sender) = sender.take()
        {
            start(app, &host, sender)?;
        }

        if app.take_restore_request() {
            read(app, &host, read_sender.clone())?;
        }
        if let Ok(read) = read_receiver.try_recv() {
            app.restore_read(read);
        }

        loop {
            match receiver.try_recv() {
                Ok(event) => app.apply(event),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    if !app.install().finished() {
                        app.apply(Event::Failed(
                            "installation stopped unexpectedly".to_owned(),
                        ));
                    }
                    break;
                }
            }
        }

        if event::poll(TICK)?
            && let TerminalEvent::Key(key) = event::read()?
        {
            app.key(key);
            keymap = follow_keyboard(host.as_ref(), app, keymap);
        }
        app.tick();
    }
}

fn follow_keyboard(host: &dyn Host, app: &App, applied: &'static str) -> &'static str {
    let requested_keymap = app.answers().keyboard.keymap;
    if requested_keymap == applied {
        return applied;
    }
    let _ = host.run(&Command::new("loadkeys").arg(requested_keymap));
    requested_keymap
}

fn start(app: &App, host: &Arc<dyn Host>, sender: mpsc::Sender<Event>) -> io::Result<()> {
    let answers = app.answers().clone();
    let host = Arc::clone(host);

    thread::Builder::new()
        .name("rikka-installer".to_owned())
        .spawn(move || {
            install(host.as_ref(), &answers, &mut |event| {
                let _ = sender.send(event);
            });
        })?;
    Ok(())
}

fn read(
    app: &App,
    host: &Arc<dyn Host>,
    sender: mpsc::Sender<Result<Restored, String>>,
) -> io::Result<()> {
    let answers = app.answers().clone();
    let host = Arc::clone(host);

    thread::Builder::new()
        .name("rikka-restore".to_owned())
        .spawn(move || {
            let _ = sender.send(read_restore(host.as_ref(), &answers));
        })?;
    Ok(())
}
