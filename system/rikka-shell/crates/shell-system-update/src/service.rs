use std::collections::HashMap;
use std::time::SystemTime;

use rikka_nix::{Event, Run, RunId};
use rikka_seam::{Call, Manifest, Method, MethodInfo, SeamClient, Service, Version};
use shell_kit::Host;

use crate::describe::{self, Shown};
use crate::watch::Inbox;

pub(super) const CAPABILITY: &str = "rikka.system-update";

pub(super) const ARRIVED: &str = "arrived";

const REMEMBERED: usize = 16;

#[derive(Debug)]
pub(super) struct SystemUpdateService {
    inbox: Inbox,
    runs: HashMap<RunId, Followed>,
}

#[derive(Debug)]
struct Followed {
    run: Run,
    shown: Option<Shown>,
}

impl SystemUpdateService {
    pub(super) fn new(inbox: Inbox) -> SystemUpdateService {
        SystemUpdateService {
            inbox,
            runs: HashMap::new(),
        }
    }

    fn arrived(&mut self, seam: &SeamClient) {
        for event in self.inbox.drain() {
            self.hear(seam, event);
        }
        self.forget_old();
    }

    fn hear(&mut self, seam: &SeamClient, event: Event) {
        match event {
            Event::Started(run) => {
                let ended = self
                    .runs
                    .get(&run.id)
                    .is_some_and(|followed| !followed.run.is_running());
                if !ended {
                    self.follow(seam, run);
                }
            }
            Event::Status { run, status } => {
                let Some(followed) = self.runs.get_mut(&run) else {
                    return;
                };
                if followed.run.is_running() {
                    followed.run.status = status;
                    self.announce(seam, run);
                }
            }
            Event::Finished(run) => self.follow(seam, run),
            Event::Exited | Event::Disconnected => {
                let now = SystemTime::now();
                let stopped: Vec<RunId> = self
                    .runs
                    .values_mut()
                    .filter(|followed| followed.run.is_running())
                    .map(|followed| {
                        followed.run.interrupt(now);
                        followed.run.id
                    })
                    .collect();
                for id in stopped {
                    self.announce(seam, id);
                }
            }
            Event::Lines(_) => {}
        }
    }

    fn follow(&mut self, seam: &SeamClient, run: Run) {
        let id = run.id;
        match self.runs.get_mut(&id) {
            Some(followed) => followed.run = run,
            None => {
                self.runs.insert(id, Followed { run, shown: None });
            }
        }
        self.announce(seam, id);
    }

    fn announce(&mut self, seam: &SeamClient, id: RunId) {
        let Some(followed) = self.runs.get_mut(&id) else {
            return;
        };
        let shown = Shown::from(&followed.run);
        if followed.shown.as_ref() == Some(&shown) {
            return;
        }
        describe::announcement(&followed.run, &shown).send(seam);
        followed.shown = Some(shown);
    }

    fn forget_old(&mut self) {
        let mut ended: Vec<RunId> = self
            .runs
            .values()
            .filter(|followed| !followed.run.is_running())
            .map(|followed| followed.run.id)
            .collect();
        if ended.len() <= REMEMBERED {
            return;
        }
        ended.sort_unstable();
        for id in &ended[..ended.len() - REMEMBERED] {
            self.runs.remove(id);
        }
    }
}

impl Service<Host> for SystemUpdateService {
    fn manifest(&self) -> Manifest {
        Manifest::new(CAPABILITY, Version::new(1, 0))
            .summary("Announces system changes applied by rikka-nixctl")
            .method(MethodInfo::new(ARRIVED).summary("A system change progressed"))
    }

    fn call(&mut self, host: &mut Host, method: Method, call: Call) {
        match &*method {
            ARRIVED => {
                self.arrived(host.seam());
                call.done();
            }
            _ => call.unknown_method(),
        }
    }
}
