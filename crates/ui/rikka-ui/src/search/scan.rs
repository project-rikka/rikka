use rikka_sync::{Mutex, MutexGuard};
use std::ops::Range;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};

use rikka_text_document::Rope;

use crate::runtime::NodeHandle;
use crate::search::query::Query;
use crate::search::state::Step;

const CANCEL_CHECK_INTERVAL: usize = 1024;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Aim {
    #[default]
    Nearest,
    Forward,
    Backward,
}

impl Aim {
    pub fn from_step(step: Step) -> Aim {
        match step {
            Step::Previous => Aim::Backward,
            _ => Aim::Forward,
        }
    }

    fn navigates(self) -> bool {
        self != Aim::Nearest
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct ScanKey {
    query: u64,
    revision: u64,
}

#[derive(Clone, Debug, Default)]
pub struct ScanResult {
    turn: u64,
    aim: Aim,
    count: usize,
    current: Option<(usize, Range<usize>)>,
}

impl ScanResult {
    pub fn count(&self) -> usize {
        self.count
    }

    pub fn current(&self) -> Option<usize> {
        self.current.as_ref().map(|(at, _)| *at)
    }

    pub fn current_range(&self) -> Option<Range<usize>> {
        self.current.as_ref().map(|(_, range)| range.clone())
    }
}

struct ScanRequest {
    turn: u64,
    query: Query,
    rope: Rope,
    from: Range<usize>,
    aim: Aim,
    requester: Option<NodeHandle>,
}

#[derive(Debug, Default)]
pub struct Scan {
    worker: Option<ScanWorker>,
    latest_turn: u64,
    latest_key: Option<ScanKey>,
    navigated_turn: u64,
    result: ScanResult,
}

impl Scan {
    pub fn request(
        &mut self,
        query: &Query,
        rope: &Rope,
        revision: u64,
        from: Range<usize>,
        aim: Aim,
    ) {
        let key = ScanKey {
            query: query.stamp(),
            revision,
        };
        if !aim.navigates() && self.latest_key == Some(key) {
            return;
        }
        self.latest_turn += 1;
        self.latest_key = Some(key);
        let request = ScanRequest {
            turn: self.latest_turn,
            query: query.clone(),
            rope: rope.clone(),
            from,
            aim,
            requester: NodeHandle::try_current(),
        };
        if self.worker.is_none() {
            self.worker = ScanWorker::start();
        }
        let request = match &self.worker {
            Some(worker) => match worker.send(request) {
                Ok(()) => return,
                Err(request) => {
                    tracing::warn!("search thread stopped, searching inline");
                    self.worker = None;
                    request
                }
            },
            None => request,
        };
        let turn = Arc::new(AtomicU64::new(request.turn));
        self.result = scan(&request, &turn, &mut String::new()).unwrap_or_default();
    }

    pub fn result(&mut self) -> &ScanResult {
        if let Some(worker) = &self.worker {
            let latest = worker.result();
            if latest.turn > self.result.turn {
                self.result = latest.clone();
            }
        }
        &self.result
    }

    pub fn is_pending(&self) -> bool {
        self.result.turn != self.latest_turn
    }

    pub fn take_navigation_target(&mut self) -> Option<Range<usize>> {
        if self.result.turn == 0
            || self.result.turn == self.navigated_turn
            || !self.result.aim.navigates()
        {
            return None;
        }
        self.navigated_turn = self.result.turn;
        self.result.current_range()
    }

    pub fn reset(&mut self) {
        *self = Scan::default();
    }
}

#[derive(Debug)]
struct ScanWorker {
    requests: Sender<ScanRequest>,
    latest_turn: Arc<AtomicU64>,
    result: Arc<Mutex<ScanResult>>,
}

impl ScanWorker {
    fn start() -> Option<ScanWorker> {
        let (requests, receiver) = channel();
        let latest_turn = Arc::new(AtomicU64::new(0));
        let result = Arc::new(Mutex::new(ScanResult::default()));

        let thread_turn = Arc::clone(&latest_turn);
        let thread_result = Arc::clone(&result);
        std::thread::Builder::new()
            .name("rikka-search".to_owned())
            .spawn(move || run(&receiver, &thread_turn, &thread_result))
            .inspect_err(|error| {
                tracing::warn!("failed to spawn search thread, searching inline: {error}");
            })
            .ok()?;

        Some(ScanWorker {
            requests,
            latest_turn,
            result,
        })
    }

    fn send(&self, request: ScanRequest) -> Result<(), ScanRequest> {
        self.latest_turn.store(request.turn, Ordering::Relaxed);
        self.requests.send(request).map_err(|error| error.0)
    }

    fn result(&self) -> MutexGuard<'_, ScanResult> {
        self.result.lock()
    }
}

fn run(
    receiver: &Receiver<ScanRequest>,
    latest_turn: &Arc<AtomicU64>,
    result: &Arc<Mutex<ScanResult>>,
) {
    let mut text = String::new();
    while let Ok(mut request) = receiver.recv() {
        while let Ok(newer) = receiver.try_recv() {
            request = newer;
        }
        if latest_turn.load(Ordering::Relaxed) != request.turn {
            continue;
        }
        let Some(scanned) = scan(&request, latest_turn, &mut text) else {
            continue;
        };
        {
            let mut stored = result.lock();
            if scanned.turn >= stored.turn {
                *stored = scanned;
            }
        }
        if let Some(requester) = &request.requester {
            requester.repaint();
        }
    }
}

fn scan(
    request: &ScanRequest,
    latest_turn: &Arc<AtomicU64>,
    text: &mut String,
) -> Option<ScanResult> {
    text.clear();
    text.extend(request.rope.chunks(0..request.rope.len()));
    let mut count = 0usize;
    let mut first: Option<Range<usize>> = None;
    let mut last: Option<(usize, Range<usize>)> = None;
    let mut ahead: Option<(usize, Range<usize>)> = None;
    let mut behind: Option<(usize, Range<usize>)> = None;

    for range in request.query.matches(text) {
        if count.is_multiple_of(CANCEL_CHECK_INTERVAL)
            && latest_turn.load(Ordering::Relaxed) != request.turn
        {
            return None;
        }
        if first.is_none() {
            first = Some(range.clone());
        }
        if ahead.is_none() {
            let reached = match request.aim {
                Aim::Forward => range.start >= request.from.end,
                Aim::Nearest | Aim::Backward => range.end > request.from.start,
            };
            if reached {
                ahead = Some((count, range.clone()));
            }
        }
        if range.end <= request.from.start {
            behind = Some((count, range.clone()));
        }
        last = Some((count, range));
        count += 1;
    }

    let current = match request.aim {
        Aim::Backward => behind.or(last),
        Aim::Forward | Aim::Nearest => ahead.or_else(|| first.map(|range| (0, range))),
    };
    Some(ScanResult {
        turn: request.turn,
        aim: request.aim,
        count,
        current,
    })
}
