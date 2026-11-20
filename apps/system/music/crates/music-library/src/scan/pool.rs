use rikka_sync::Mutex;
use std::sync::mpsc;
use std::thread;

use crate::artwork::ArtworkCache;
use crate::ids::TrackId;
use crate::index::Stat;
use crate::scan::halt::Halt;
use crate::scan::reading;

pub(crate) use reading::Outcome;

const MAX_WORKERS: usize = 8;
const QUEUED: usize = 64;

pub(crate) fn read_all(
    items: Vec<(usize, Option<TrackId>, Stat)>,
    artwork: &ArtworkCache,
    halt: Halt<'_>,
    mut each: impl FnMut(usize, Option<TrackId>, Outcome),
) {
    if items.is_empty() {
        return;
    }
    let workers = thread::available_parallelism()
        .map_or(1, usize::from)
        .clamp(1, MAX_WORKERS)
        .min(items.len());
    let queue = Mutex::new(items.into_iter());
    let (sender, results) = mpsc::sync_channel(QUEUED);
    thread::scope(|scope| {
        for _ in 0..workers {
            let sender = sender.clone();
            let queue = &queue;
            let spawned = thread::Builder::new()
                .name("rikka-music-tags".into())
                .spawn_scoped(scope, move || {
                    while !halt.is_set() {
                        let next = queue.lock().next();
                        let Some((index, id, stat)) = next else {
                            break;
                        };
                        if sender
                            .send((index, id, reading::read(stat, artwork)))
                            .is_err()
                        {
                            break;
                        }
                    }
                });
            if let Err(error) = spawned {
                tracing::warn!("failed to spawn tag reader: {error}");
            }
        }
        drop(sender);
        for (index, id, outcome) in results {
            each(index, id, outcome);
        }
    });
}
