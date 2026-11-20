use rikka_sync::{Mutex, MutexGuard};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::os::unix::fs::FileExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock, Weak};

use crate::store::{BLOBS, JOURNAL, Line, restored};
use crate::turn::{Body, Speaker};

#[derive(serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum Marked {
    Turn(Mark),
    Running(serde::de::IgnoredAny),
    Shed,
}

#[derive(serde::Deserialize)]
struct Mark {
    speaker: Speaker,
    at: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Placement {
    pub(crate) index: usize,
    pub(crate) at: u64,
    pub(crate) len: u32,
}

#[derive(Debug)]
pub(crate) struct Journal {
    at: PathBuf,
    blobs: PathBuf,
    placements: Mutex<Option<Vec<Placement>>>,
}

impl Journal {
    pub(crate) fn open(folder: &Path) -> Arc<Journal> {
        let mut known = journals();
        known.retain(|_, journal| journal.strong_count() > 0);
        if let Some(shared) = known.get(folder).and_then(Weak::upgrade) {
            return shared;
        }
        let journal = Arc::new(Journal {
            at: folder.join(JOURNAL),
            blobs: folder.join(BLOBS),
            placements: Mutex::new(None),
        });
        known.insert(folder.to_path_buf(), Arc::downgrade(&journal));
        journal
    }

    pub(crate) fn read(
        &self,
        placement: Placement,
        speaker: Speaker,
        at: u64,
    ) -> Option<(Body, Placement)> {
        let mut tried = placement;
        loop {
            if let Some(body) = self.read_at(tried, speaker, at) {
                return Some((body, tried));
            }
            let found = self.placed_again(tried)?;
            if found == tried {
                return None;
            }
            tried = found;
        }
    }

    pub(crate) fn line(&self, placement: Placement, speaker: Speaker, at: u64) -> Option<Vec<u8>> {
        let bytes = self.bytes(placement)?;
        match serde_json::from_slice::<Marked>(&bytes).ok()? {
            Marked::Turn(mark) if mark.at == at && mark.speaker == speaker => Some(bytes),
            _ => None,
        }
    }

    pub(crate) fn note(&self, placements: Vec<Placement>) {
        *self.placements() = Some(placements);
    }

    fn read_at(&self, placement: Placement, speaker: Speaker, at: u64) -> Option<Body> {
        let bytes = self.bytes(placement)?;
        let line = match serde_json::from_slice::<Line>(&bytes).ok()? {
            Line::Turn(line) | Line::Running(line) => line,
            Line::Shed => return None,
        };
        if line.at != at || line.speaker != speaker {
            return None;
        }
        Some(restored(line, &self.blobs))
    }

    fn bytes(&self, placement: Placement) -> Option<Vec<u8>> {
        let file = File::open(&self.at).ok()?;
        let mut bytes = vec![0_u8; placement.len as usize];
        file.read_exact_at(&mut bytes, placement.at).ok()?;
        Some(bytes)
    }

    fn placed_again(&self, placement: Placement) -> Option<Placement> {
        let known = self
            .placements()
            .as_ref()
            .and_then(|all| all.get(placement.index).copied());
        if let Some(known) = known.filter(|known| *known != placement) {
            return Some(known);
        }
        let all = walked(&self.at)?;
        let found = all.get(placement.index).copied();
        *self.placements() = Some(all);
        found
    }

    fn placements(&self) -> MutexGuard<'_, Option<Vec<Placement>>> {
        self.placements.lock()
    }
}

pub(crate) fn walked(at: &Path) -> Option<Vec<Placement>> {
    let journal = File::open(at).ok()?;
    let mut placements = Vec::new();
    let mut running = None;
    let mut offset = 0_u64;
    for line in BufReader::new(journal).split(b'\n') {
        let line = line.ok()?;
        let at = offset;
        offset += line.len() as u64 + 1;
        let placement = Placement {
            index: placements.len(),
            at,
            len: line.len() as u32,
        };
        match serde_json::from_slice::<Line>(&line) {
            Ok(Line::Turn(_)) => {
                running = None;
                placements.push(placement);
            }
            Ok(Line::Running(_)) => running = Some(placement),
            Ok(Line::Shed) => running = None,
            Err(_) => {}
        }
    }
    if let Some(mut running) = running {
        running.index = placements.len();
        placements.push(running);
    }
    Some(placements)
}

fn journals() -> MutexGuard<'static, HashMap<PathBuf, Weak<Journal>>> {
    static KNOWN: OnceLock<Mutex<HashMap<PathBuf, Weak<Journal>>>> = OnceLock::new();
    KNOWN.get_or_init(|| Mutex::new(HashMap::new())).lock()
}
