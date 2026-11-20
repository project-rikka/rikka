use std::hash::Hash;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, TryRecvError, channel};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use rikka_sync::{Mutex, MutexGuard};

use rikka_renderer::{
    Acquired, Error, FrameStart, Preparer, Present, Recorded, Renderer, Scene, SceneFrame,
    TRIM_AFTER, Uploads,
};
use rustc_hash::FxHashMap;

use rikka_picture::Waking;

pub type Presenting<S> = Arc<dyn Fn(&mut S) + Send + Sync>;

const SLICE: Duration = Duration::from_millis(4);

const LONGEST: Duration = Duration::from_secs(1);

const ATTEMPTS: usize = 4;

enum Order<W> {
    Frame(Box<SceneFrame>),
    Present(Box<W>),
}

pub(crate) struct PaintResult<K, W> {
    pub(crate) key: K,
    pub(crate) surface: W,
    pub(crate) recorded: Option<Recorded>,
    pub(crate) again: bool,
    pub(crate) settling: bool,
}

struct Hand<W> {
    orders: Sender<Order<W>>,
    returned: Receiver<Box<SceneFrame>>,
    closing: Arc<AtomicBool>,
    thread: JoinHandle<()>,
}

impl<W> Hand<W> {
    fn stop(&self) {
        self.closing.store(true, Ordering::Release);
    }

    fn part(self) -> JoinHandle<()> {
        self.stop();
        self.thread
    }
}

pub(crate) struct Painter<K, W> {
    renderer: Arc<Mutex<Renderer>>,
    uploads: Uploads,
    hands: FxHashMap<K, Hand<W>>,
    done: Sender<PaintResult<K, W>>,
    painted: Receiver<PaintResult<K, W>>,
    wake: Option<Waking>,
    parting: Vec<JoinHandle<()>>,
}

impl<K: Eq + Hash + Clone + Send + 'static, W: Present> Painter<K, W> {
    pub(crate) fn start(renderer: Renderer, wake: Option<Waking>) -> Painter<K, W> {
        let (done, painted) = channel::<PaintResult<K, W>>();
        Painter {
            uploads: renderer.uploads(),
            renderer: Arc::new(Mutex::new(renderer)),
            hands: FxHashMap::default(),
            done,
            painted,
            wake,
            parting: Vec::new(),
        }
    }

    pub(crate) fn renderer(&self) -> MutexGuard<'_, Renderer> {
        self.renderer.lock()
    }

    pub(crate) fn uploads(&self) -> &Uploads {
        &self.uploads
    }

    pub(crate) fn shared_renderer(&self) -> Arc<Mutex<Renderer>> {
        Arc::clone(&self.renderer)
    }

    pub(crate) fn spent_frame(&self, key: &K) -> Box<SceneFrame> {
        self.hands
            .get(key)
            .and_then(|hand| hand.returned.try_recv().ok())
            .unwrap_or_default()
    }

    pub(crate) fn send_frame(&mut self, key: &K, frame: Box<SceneFrame>) -> bool {
        self.order(key, Order::Frame(frame)).is_ok()
    }

    pub(crate) fn present(&mut self, key: &K, surface: W) -> Result<(), Box<W>> {
        match self.order(key, Order::Present(Box::new(surface))) {
            Ok(()) => Ok(()),
            Err(refused) => match *refused {
                Order::Present(surface) => Err(surface),
                Order::Frame(_) => unreachable!("rejected paint request returned unchanged"),
            },
        }
    }

    fn order(&mut self, key: &K, order: Order<W>) -> Result<(), Box<Order<W>>> {
        self.reap();
        let hand = match self.hands.get(key) {
            Some(hand) => hand,
            None => match self.hire(key.clone()) {
                Ok(hand) => hand,
                Err(error) => {
                    tracing::error!("failed to spawn paint thread: {error}");
                    return Err(Box::new(order));
                }
            },
        };
        match hand.orders.send(order) {
            Ok(()) => Ok(()),
            Err(refused) => {
                self.retire(key);
                Err(Box::new(refused.0))
            }
        }
    }

    fn hire(&mut self, key: K) -> std::io::Result<&Hand<W>> {
        let (orders, receiver) = channel::<Order<W>>();
        let (spent, returned) = channel::<Box<SceneFrame>>();
        let closing = Arc::new(AtomicBool::new(false));
        let renderer = Arc::clone(&self.renderer);
        let done = self.done.clone();
        let wake = self.wake.clone();
        let mine = Arc::clone(&closing);
        let named = key.clone();
        let thread = std::thread::Builder::new()
            .name(String::from("rikka-ui-painter"))
            .spawn(move || {
                let preparer = renderer.lock().preparer();
                paint(Painting {
                    key: named,
                    renderer: &renderer,
                    preparer,
                    receiver: &receiver,
                    done: &done,
                    spent,
                    wake: wake.as_ref(),
                    closing: &mine,
                });
            })?;
        Ok(self
            .hands
            .entry(key)
            .insert_entry(Hand {
                orders,
                returned,
                closing,
                thread,
            })
            .into_mut())
    }

    pub(crate) fn stop(&self, key: &K) {
        if let Some(hand) = self.hands.get(key) {
            hand.stop();
        }
    }

    pub(crate) fn retire(&mut self, key: &K) {
        if let Some(hand) = self.hands.remove(key) {
            self.parting.push(hand.part());
        }
        self.reap();
    }

    pub(crate) fn next_painted(&self, while_painting: &K) -> Option<PaintResult<K, W>> {
        loop {
            match self.painted.recv_timeout(SLICE) {
                Ok(one) => return Some(one),
                Err(RecvTimeoutError::Disconnected) => return None,
                Err(RecvTimeoutError::Timeout) => {
                    let painting = self
                        .hands
                        .get(while_painting)
                        .is_some_and(|hand| !hand.thread.is_finished());
                    if !painting {
                        return None;
                    }
                }
            }
        }
    }

    pub(crate) fn take_painted(&self) -> Vec<PaintResult<K, W>> {
        let mut painted = Vec::new();
        loop {
            match self.painted.try_recv() {
                Ok(one) => painted.push(one),
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => return painted,
            }
        }
    }

    fn reap(&mut self) {
        let mut index = 0;
        while index < self.parting.len() {
            if self.parting[index].is_finished() {
                let _ = self.parting.swap_remove(index).join();
                continue;
            }
            index += 1;
        }
    }
}

impl<K, W> Drop for Painter<K, W> {
    fn drop(&mut self) {
        let parting: Vec<JoinHandle<()>> = self
            .hands
            .drain()
            .map(|(_, hand)| hand.part())
            .chain(self.parting.drain(..))
            .collect();
        for thread in parting {
            let _ = thread.join();
        }
    }
}

struct Painting<'a, K, W> {
    key: K,
    renderer: &'a Mutex<Renderer>,
    preparer: Preparer,
    receiver: &'a Receiver<Order<W>>,
    done: &'a Sender<PaintResult<K, W>>,
    spent: Sender<Box<SceneFrame>>,
    wake: Option<&'a Waking>,
    closing: &'a AtomicBool,
}

fn paint<K: Clone, W: Present>(mut painting: Painting<'_, K, W>) {
    let mut mirror = Scene::new();
    let mut drew = None;
    while let Some(first) = next_order(painting.receiver, painting.renderer, &mut drew) {
        let mut received = false;
        let mut present = None;
        let mut next = Some(first);
        while let Some(order) = next.take() {
            match order {
                Order::Frame(mut frame) => {
                    mirror.apply(&mut frame);
                    let _ = painting.spent.send(frame);
                    received = true;
                    next = painting.receiver.try_recv().ok();
                }
                Order::Present(surface) => present = Some(surface),
            }
        }
        if received && let Err(error) = painting.preparer.prepare(&mut mirror) {
            tracing::error!("failed to prepare frame: {error}");
        }
        let Some(mut surface) = present else {
            continue;
        };
        let (recorded, again) = match record(
            painting.renderer,
            &mut mirror,
            &mut *surface,
            painting.closing,
        ) {
            Ok(Outcome::Drawn(recorded)) => (Some(recorded), false),
            Ok(Outcome::Waiting) => (None, true),
            Err(error) => {
                tracing::error!("failed to paint frame: {error}");
                (None, false)
            }
        };
        if recorded.as_ref().is_some_and(Recorded::is_presentable) {
            drew = Some(Instant::now());
        }
        let sent = painting.done.send(PaintResult {
            key: painting.key.clone(),
            surface: *surface,
            recorded,
            again,
            settling: mirror.is_animating(),
        });
        if sent.is_err() {
            return;
        }
        if let Some(wake) = painting.wake {
            wake();
        }
    }
}

fn next_order<W>(
    receiver: &Receiver<Order<W>>,
    renderer: &Mutex<Renderer>,
    drew: &mut Option<Instant>,
) -> Option<Order<W>> {
    loop {
        let Some(at) = *drew else {
            return receiver.recv().ok();
        };
        let left = (at + TRIM_AFTER).saturating_duration_since(Instant::now());
        match receiver.recv_timeout(left) {
            Ok(order) => return Some(order),
            Err(RecvTimeoutError::Timeout) => {
                renderer.lock().trim();
                *drew = None;
            }
            Err(RecvTimeoutError::Disconnected) => return None,
        }
    }
}

enum Outcome {
    Drawn(Recorded),
    Waiting,
}

fn record<W: Present>(
    renderer: &Mutex<Renderer>,
    scene: &mut Scene,
    surface: &mut W,
    closing: &AtomicBool,
) -> rikka_renderer::Result<Outcome> {
    for _ in 0..ATTEMPTS {
        match surface.start(&mut renderer.lock(), scene)? {
            FrameStart::Idle => return Ok(Outcome::Drawn(Recorded::default())),
            FrameStart::Deferred => return Ok(Outcome::Drawn(Recorded::deferred())),
            FrameStart::Draw => {}
        }
        match acquire_image(surface, closing)? {
            Acquired::Image(image) => {
                wait_for_frame_slot(renderer)?;
                return surface
                    .record_image(&mut renderer.lock(), scene, image)
                    .map(Outcome::Drawn);
            }
            Acquired::NotReady => return Ok(Outcome::Waiting),
            Acquired::Stale => {}
        }
    }
    Err(Error::Unsettled)
}

fn wait_for_frame_slot(renderer: &Mutex<Renderer>) -> rikka_renderer::Result<()> {
    let (timeline, due) = {
        let renderer = renderer.lock();
        (renderer.timeline(), renderer.next_free_frame())
    };
    match due {
        Some(due) => timeline.wait(due),
        None => Ok(()),
    }
}

fn acquire_image<W: Present>(
    surface: &mut W,
    closing: &AtomicBool,
) -> rikka_renderer::Result<Acquired<W::Image>> {
    let until = Instant::now() + LONGEST;
    loop {
        if closing.load(Ordering::Acquire) {
            return Ok(Acquired::NotReady);
        }
        match surface.acquire_image(SLICE)? {
            Acquired::NotReady if Instant::now() < until => {}
            acquired => return Ok(acquired),
        }
    }
}
