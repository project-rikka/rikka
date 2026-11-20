use std::time::SystemTime;

use rikka_app::prelude::*;
use rikka_dialog::{Chooser, ChooserResponse};
use rikka_nix::{Event, Interest, Monitor, Run, RunId};

use crate::describe;
use crate::log::Log;

type Watched = (Monitor, rikka_nix::Result<Vec<Run>>);

const RUN_FLAG: &str = "--run";

#[derive(Debug)]
pub(crate) enum Link {
    Connecting,
    Ready,
    Failed(String),
}

#[derive(State)]
pub(crate) struct Updates {
    link: Link,
    monitor: Option<Monitor>,
    runs: Vec<Run>,
    selected: Option<RunId>,
    wanted: Option<RunId>,
    notice: Option<Label>,
    #[state]
    log: Log,
    sender: Sender<Updates>,
}

impl Updates {
    pub(crate) fn new(sender: Sender<Updates>) -> Self {
        Self {
            link: Link::Connecting,
            monitor: None,
            runs: Vec::new(),
            selected: None,
            wanted: requested_run(&LaunchRequest::from_arguments(std::env::args_os())),
            notice: None,
            log: Log::new(),
            sender,
        }
    }

    pub(crate) fn link(&self) -> &Link {
        &self.link
    }

    pub(crate) fn runs(&self) -> &[Run] {
        &self.runs
    }

    pub(crate) fn log(&self) -> &Log {
        &self.log
    }

    pub(crate) fn notice(&self) -> Option<&Label> {
        self.notice.as_ref()
    }

    pub(crate) fn selected(&self) -> Option<&Run> {
        let id = self.selected?;
        self.runs.iter().find(|run| run.id == id)
    }

    pub(crate) fn selected_index(&self) -> Option<usize> {
        let id = self.selected?;
        self.runs.iter().position(|run| run.id == id)
    }

    pub(crate) fn start(&mut self) {
        self.link = Link::Connecting;
        let watching = self.monitor.clone();
        let sender = self.sender.clone();
        spawn_blocking(
            move || {
                let monitor = match watching {
                    Some(monitor) => monitor,
                    None => Monitor::watch(Interest::Transcript, move |event| {
                        sender
                            .send(move |updates: &mut Updates| updates.hear(event))
                            .ok();
                    })?,
                };
                let runs = monitor.runs();
                Ok((monitor, runs))
            },
            |updates: &mut Updates, watched: rikka_nix::Result<Watched>| match watched {
                Ok((monitor, runs)) => updates.linked(monitor, runs),
                Err(error) => updates.link = Link::Failed(error.to_string()),
            },
        );
    }

    pub(crate) fn opened_with(&mut self, open: LaunchRequest) {
        let Some(id) = requested_run(&open) else {
            return skip_view();
        };
        match self.runs.iter().any(|run| run.id == id) {
            true => self.choose(id),
            false => self.wanted = Some(id),
        }
    }

    pub(crate) fn select(&mut self, index: usize) {
        if let Some(id) = self.runs.get(index).map(|run| run.id) {
            self.choose(id);
        }
    }

    pub(crate) fn dismiss_notice(&mut self) {
        self.notice = None;
    }

    pub(crate) fn copy_log(&mut self) {
        rikka_ui::clipboard::set_text(&self.log.text());
        skip_view();
    }

    pub(crate) fn save_log(&mut self) {
        let Some(run) = self.selected() else {
            return;
        };
        let chooser = Chooser::save(tr!("updates.save-title").to_string())
            .parent(window_identifier())
            .current_name(describe::file_name(run));
        let text = self.log.text();
        spawn_blocking(
            move || match chooser.run() {
                Ok(ChooserResponse::Files(paths)) => match paths.into_iter().next() {
                    Some(path) => std::fs::write(path, text).map_err(|error| error.to_string()),
                    None => Ok(()),
                },
                Ok(ChooserResponse::Cancelled) => Ok(()),
                Err(error) => Err(error.to_string()),
            },
            |updates: &mut Updates, saved: Result<(), String>| {
                updates.notice = saved
                    .err()
                    .map(|reason| tr!("updates.unsaved", reason = &reason));
            },
        );
    }

    fn linked(&mut self, monitor: Monitor, runs: rikka_nix::Result<Vec<Run>>) {
        self.monitor = Some(monitor);
        let runs = match runs {
            Ok(runs) => runs,
            Err(error) => {
                self.link = Link::Failed(error.to_string());
                return;
            }
        };
        let heard = std::mem::replace(&mut self.runs, runs);
        for run in heard {
            self.keep(run);
        }
        self.link = Link::Ready;
        let chosen = self
            .wanted
            .take()
            .filter(|id| self.runs.iter().any(|run| run.id == *id))
            .or_else(|| self.runs.first().map(|run| run.id));
        if let Some(id) = chosen {
            self.choose(id);
        }
    }

    fn hear(&mut self, event: Event) {
        match event {
            Event::Started(run) => {
                let follows = self.selected.is_none() || self.selected == self.newest();
                let id = run.id;
                self.keep(run);
                if follows {
                    self.choose(id);
                }
            }
            Event::Status { run, status } => {
                match self.runs.iter_mut().find(|known| known.id == run) {
                    Some(known) if known.is_running() => known.status = status,
                    _ => skip_view(),
                }
            }
            Event::Finished(run) => self.keep(run),
            Event::Lines(lines) => {
                if let Some(monitor) = &self.monitor {
                    self.log.receive(lines, monitor);
                }
                skip_view();
            }
            Event::Exited => self.interrupt(),
            Event::Disconnected => {
                self.interrupt();
                self.monitor = None;
                self.link = Link::Failed(tr!("updates.disconnected").to_string());
            }
        }
    }

    fn interrupt(&mut self) {
        let now = SystemTime::now();
        for run in &mut self.runs {
            run.interrupt(now);
        }
    }

    fn keep(&mut self, run: Run) {
        match self.runs.iter_mut().find(|known| known.id == run.id) {
            Some(known) if known.is_running() || !run.is_running() => *known = run,
            Some(_) => {}
            None => {
                let at = self
                    .runs
                    .iter()
                    .position(|known| known.id < run.id)
                    .unwrap_or(self.runs.len());
                self.runs.insert(at, run);
            }
        }
    }

    fn choose(&mut self, id: RunId) {
        self.selected = Some(id);
        if let Some(monitor) = &self.monitor {
            self.log.show(id, monitor);
        }
    }

    fn newest(&self) -> Option<RunId> {
        self.runs.first().map(|run| run.id)
    }
}

fn requested_run(open: &LaunchRequest) -> Option<RunId> {
    let arguments = open.arguments();
    let at = arguments.iter().position(|argument| argument == RUN_FLAG)?;
    arguments.get(at + 1)?.to_str()?.parse().ok()
}

impl std::fmt::Debug for Updates {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Updates")
            .field("link", &self.link)
            .field("runs", &self.runs.len())
            .field("selected", &self.selected)
            .finish_non_exhaustive()
    }
}
