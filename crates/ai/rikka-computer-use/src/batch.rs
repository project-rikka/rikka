use rikka_sync::Mutex;
use std::time::Duration;

use rikka_computer::{Computer, Shot, Status};
use rikka_tool::{Image, Media, Reply};
use rikka_types::{Modifiers, PointerButton};

use crate::action::{Action, Click, Effect, Step};

#[derive(Clone, Copy, Debug)]
pub(crate) struct Settling {
    pub(crate) idle: Duration,
    pub(crate) window: Duration,
}

#[derive(Debug, Default)]
pub(crate) struct Seen {
    previous_call: Mutex<Option<u64>>,
}

impl Seen {
    fn take(&self) -> Option<u64> {
        self.previous_call.lock().take()
    }

    fn keep(&self, screen: Option<u64>) {
        *self.previous_call.lock() = screen;
    }
}

#[derive(Debug)]
enum Settle {
    Idle,
    NewWindow { known_windows: Vec<String> },
}

enum Outcome {
    Text(String),
    Image { text: String, shot: Shot },
}

#[derive(Default)]
struct Report {
    lines: Vec<String>,
    images: Vec<Image>,
    failed: bool,
}

impl Report {
    fn line(&mut self, line: String) {
        self.lines.push(line);
    }

    fn image(&mut self, shot: Shot) {
        self.images.push(Image::inline(Media::Png, shot.into_png()));
    }

    fn fail(&mut self, labels: &[String], index: usize, error: &str) {
        self.failed = true;
        self.line(format!("{} failed: {error}", labels[index]));
        for label in &labels[index + 1..] {
            self.line(format!("{label} not run: an earlier action failed"));
        }
    }

    fn into_reply(self) -> Reply {
        let reply = Reply::new(self.lines.join("\n"));
        let reply = self.images.into_iter().fold(reply, Reply::image);
        match self.failed {
            true => reply.failed(),
            false => reply,
        }
    }
}

pub(crate) fn run(computer: &Computer, steps: &[Step], settling: Settling, seen: &Seen) -> Reply {
    let labels: Vec<String> = match steps {
        [step] => vec![step.name.clone()],
        steps => steps
            .iter()
            .enumerate()
            .map(|(index, step)| format!("{}. {}", index + 1, step.name))
            .collect(),
    };

    let previous_call = seen.take();
    let mut screen = None;
    let mut report = Report::default();
    let mut pending = None;
    for (index, step) in steps.iter().enumerate() {
        if let Some(settle) = pending.take()
            && let Some(note) = settle_screen(computer, settle, settling)
        {
            report.line(note);
        }
        let settle = match step.action.effect() {
            Effect::Changes => Some(Settle::Idle),
            Effect::Opens => match known_windows(computer) {
                Ok(known_windows) => Some(Settle::NewWindow { known_windows }),
                Err(error) => {
                    report.fail(&labels, index, &error);
                    break;
                }
            },
            Effect::Observes | Effect::Reports => None,
        };
        match perform(computer, &step.action) {
            Ok(Outcome::Text(text)) => report.line(described(&labels[index], &text)),
            Ok(Outcome::Image { text, shot }) => {
                if matches!(step.action, Action::Screenshot) {
                    screen = Some(shot.fingerprint());
                }
                report.line(described(&labels[index], &text));
                report.image(shot);
            }
            Err(error) => {
                report.fail(&labels, index, &error);
                break;
            }
        }
        pending = settle;
    }

    if let Some(settle) = pending {
        if let Some(note) = settle_screen(computer, settle, settling) {
            report.line(note);
        }
        match computer.screenshot() {
            Ok(shot) if Some(shot.fingerprint()) != screen.or(previous_call) => {
                screen = Some(shot.fingerprint());
                report.line(screen_size(&shot));
                report.image(shot);
            }
            Ok(_) => report.line("screen unchanged since the last screenshot".to_owned()),
            Err(error) => report.line(format!("(failed to take screenshot: {error})")),
        }
    }
    seen.keep(screen);
    report.into_reply()
}

fn described(label: &str, text: &str) -> String {
    match text.is_empty() {
        true => label.to_owned(),
        false if text.contains('\n') => format!("{label}:\n{text}"),
        false => format!("{label}: {text}"),
    }
}

fn perform(computer: &Computer, action: &Action) -> std::result::Result<Outcome, String> {
    let text = |text: &str| Ok(Outcome::Text(text.to_owned()));

    match action {
        Action::Start { display, shell } => {
            let status = computer.start(*display, *shell).map_err(to_message)?;
            Ok(Outcome::Text(describe_status(status)))
        }
        Action::Close => {
            computer.close();
            text("machine shut down")
        }
        Action::Screenshot => {
            let shot = computer.screenshot().map_err(to_message)?;
            Ok(Outcome::Image {
                text: screen_size(&shot),
                shot,
            })
        }
        Action::Zoom(region) => {
            let shot = computer.zoom(*region).map_err(to_message)?;
            Ok(Outcome::Image {
                text: format!("{region} shown at {}x{}", shot.width(), shot.height()),
                shot,
            })
        }
        Action::Windows => Ok(Outcome::Text(list_windows(computer)?)),
        Action::CursorPosition => {
            let (x, y) = computer.cursor_position().map_err(to_message)?;
            Ok(Outcome::Text(format!("pointer at ({x}, {y})")))
        }

        Action::MoveTo((x, y)) => {
            computer.move_to(*x, *y).map_err(to_message)?;
            text("")
        }
        Action::Click(Click {
            button,
            count,
            modifiers,
            at,
        }) => {
            computer
                .click(*button, *count, *modifiers, *at)
                .map_err(to_message)?;
            text("")
        }
        Action::MouseDown(click) => {
            computer
                .hold_modifiers(click.modifiers)
                .map_err(to_message)?;
            computer.press(click.button, click.at).map_err(to_message)?;
            text("")
        }
        Action::MouseUp(click) => {
            computer
                .release(click.button, click.at)
                .map_err(to_message)?;
            computer
                .hold_modifiers(Modifiers::NONE)
                .map_err(to_message)?;
            text("")
        }
        Action::Drag {
            from,
            to,
            modifiers,
        } => {
            if let Some((x, y)) = from {
                computer.move_to(*x, *y).map_err(to_message)?;
            }
            computer
                .drag(PointerButton::Primary, *to, *modifiers)
                .map_err(to_message)?;
            text("")
        }
        Action::Scroll {
            direction,
            amount,
            modifiers,
            at,
        } => {
            let (dx, dy) = direction.delta(*amount);
            computer
                .scroll(dx, dy, *modifiers, *at)
                .map_err(to_message)?;
            text("")
        }

        Action::Key { chord, repeat } => {
            for _ in 0..*repeat {
                computer.key(*chord).map_err(to_message)?;
            }
            text("")
        }
        Action::HoldKey { chord, duration } => {
            computer.hold_key(*chord, *duration).map_err(to_message)?;
            text("")
        }
        Action::Type(typed) => {
            computer.type_text(typed).map_err(to_message)?;
            text("")
        }
        Action::Wait(duration) => {
            std::thread::sleep(*duration);
            text("")
        }
        Action::Launch(command) => {
            let pid = computer.launch(command).map_err(to_message)?;
            Ok(Outcome::Text(format!("started process {pid}")))
        }
    }
}

fn known_windows(computer: &Computer) -> std::result::Result<Vec<String>, String> {
    Ok(computer
        .windows()
        .map_err(to_message)?
        .into_iter()
        .map(|window| window.identifier)
        .collect())
}

fn settle_screen(computer: &Computer, settle: Settle, settling: Settling) -> Option<String> {
    let waited = match settle {
        Settle::Idle => computer.wait_idle(settling.idle),
        Settle::NewWindow { known_windows } => computer
            .wait_for_window(
                |window| !known_windows.contains(&window.identifier),
                settling.window,
            )
            .and_then(|_| computer.wait_idle(settling.idle)),
    };
    match waited {
        Ok(())
        | Err(rikka_computer::Error::WindowTimeout(_) | rikka_computer::Error::IdleTimeout(_)) => {
            None
        }
        Err(error) => Some(format!("(failed to wait for screen updates: {error})")),
    }
}

fn screen_size(shot: &Shot) -> String {
    format!("screen {}x{}", shot.width(), shot.height())
}

fn describe_status(status: Status) -> String {
    match status.display() {
        Some(display) => format!("machine running with a {display} screen"),
        None => "machine failed to start".to_owned(),
    }
}

fn list_windows(computer: &Computer) -> std::result::Result<String, String> {
    let windows = computer.windows().map_err(to_message)?;
    if windows.is_empty() {
        return Ok("no windows open".to_owned());
    }

    Ok(windows
        .iter()
        .map(|window| format!("{} — {}", window.app_id, window.title))
        .collect::<Vec<_>>()
        .join("\n"))
}

fn to_message(error: rikka_computer::Error) -> String {
    error.to_string()
}
