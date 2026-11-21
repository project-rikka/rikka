use std::any::Any;
use std::cell::RefCell;
use std::future::Future;
use std::hash::Hash;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use futures_util::future::{BoxFuture, FutureExt};
use futures_util::stream::{BoxStream, Stream, StreamExt};
use rikka_animation::Motion;
use rikka_input::Command;

use crate::task::{Handler, IntoHandler};
use crate::view::{Chain, Scoped, StateAccess, hash_of};
use crate::window::WindowRequest;

pub type Output = Box<dyn Any + Send>;

pub enum Work {
    Future(BoxFuture<'static, Output>),
    Stream(BoxStream<'static, Output>),
    Blocking(Box<dyn FnOnce() -> Output + Send>),
}

impl std::fmt::Debug for Work {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Work::Future(_) => "Future",
            Work::Stream(_) => "Stream",
            Work::Blocking(_) => "Blocking",
        })
    }
}

#[derive(Debug)]
pub enum Job {
    Start {
        id: u64,
        key: Option<u64>,
        work: Work,
    },
    Cancel(u64),
}

#[derive(Debug)]
pub struct JobResult {
    pub job: u64,
    pub output: Option<Output>,
}

#[derive(Debug)]
pub enum RuntimeMessage {
    JobResult(JobResult),
    Update(Update),
}

pub struct Update(Box<dyn FnOnce(&mut StateAccess<'_>) + Send>);

impl Update {
    pub fn new<S: 'static>(update: impl FnOnce(&mut S) + Send + 'static) -> Self {
        Self(Box::new(move |reach| {
            if let Some(state) = reach.get::<S>() {
                update(state);
            }
        }))
    }

    pub(crate) fn apply(self, reach: &mut StateAccess<'_>) {
        (self.0)(reach);
    }
}

impl std::fmt::Debug for Update {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Update").finish_non_exhaustive()
    }
}

type Deliver = Rc<dyn Fn(&mut StateAccess<'_>, Output)>;

pub(crate) struct Pending {
    pub(crate) key: Option<u64>,
    pub(crate) lasting: bool,
    pub(crate) chain: Vec<Rc<Scoped>>,
    pub(crate) deliver: Deliver,
}

#[derive(Default)]
pub(crate) struct Effects {
    pub(crate) acted: bool,
    pub(crate) plain: bool,
    pub(crate) skip_view: bool,
    pub(crate) frame: bool,
    pub(crate) animation: Option<Motion>,
    pub(crate) jobs: Vec<Job>,
    pub(crate) pending: Vec<(u64, Pending)>,
    pub(crate) windows: Vec<WindowRequest>,
    pub(crate) commands: Vec<Command>,
    chain: Vec<Rc<Scoped>>,
}

impl Effects {
    pub(crate) fn wants_view(&self) -> bool {
        self.acted && self.plain && !self.skip_view && !self.frame
    }
}

thread_local! {
    static COLLECTING: RefCell<Vec<Effects>> = const { RefCell::new(Vec::new()) };
}

static JOBS: AtomicU64 = AtomicU64::new(1);

pub(crate) fn collecting<R>(body: impl FnOnce() -> R) -> (R, Effects) {
    COLLECTING.with(|stack| stack.borrow_mut().push(Effects::default()));
    let result = body();
    let effects = COLLECTING
        .with(|stack| stack.borrow_mut().pop())
        .expect("effects stack is empty");
    (result, effects)
}

struct Enclosing {
    acted: bool,
    plain: bool,
    skip_view: bool,
    chain: Vec<Rc<Scoped>>,
}

pub(crate) fn acting<R>(chain: &Chain, acts: Acts, body: impl FnOnce() -> R) -> R {
    let enclosing = with_effects(|effects| Enclosing {
        acted: std::mem::replace(&mut effects.acted, acts != Acts::Relaying),
        plain: std::mem::replace(&mut effects.plain, acts == Acts::Plainly),
        skip_view: std::mem::take(&mut effects.skip_view),
        chain: std::mem::replace(&mut effects.chain, chain.to_vec()),
    });
    let result = body();
    if let Some(enclosing) = enclosing {
        with_effects(|effects| {
            let kept = !effects.skip_view;
            effects.acted = enclosing.acted || (kept && effects.acted);
            effects.plain = enclosing.plain || (kept && effects.plain);
            effects.skip_view = enclosing.skip_view;
            effects.chain = enclosing.chain;
        });
    }
    result
}

fn with_effects<R>(apply: impl FnOnce(&mut Effects) -> R) -> Option<R> {
    COLLECTING.with(|stack| stack.borrow_mut().last_mut().map(apply))
}

fn effects<R>(what: &str, apply: impl FnOnce(&mut Effects) -> R) -> R {
    with_effects(apply)
        .unwrap_or_else(|| panic!("`{what}` called outside an action, start or update handler"))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Acts {
    Relaying,
    Tracked,
    Plainly,
}

pub(crate) fn handling() -> bool {
    COLLECTING.with(|stack| !stack.borrow().is_empty())
}

pub(crate) fn changed() {
    with_effects(|effects| {
        effects.acted = true;
        effects.plain = true;
    });
}

pub(crate) fn acted() {
    with_effects(|effects| effects.acted = true);
}

pub(crate) fn plain() {
    with_effects(|effects| effects.plain = true);
}

pub fn skip_view() {
    effects("skip_view", |effects| effects.skip_view = true);
}

pub fn request_frame() {
    effects("request_frame", |effects| effects.frame = true);
}

pub fn with_animation<R>(motion: Motion, body: impl FnOnce() -> R) -> R {
    effects("with_animation", |effects| effects.animation = Some(motion));
    body()
}

pub fn request_window(request: WindowRequest) {
    effects("request_window", |effects| effects.windows.push(request));
}

pub fn send_command(command: Command) {
    effects("send_command", |effects| effects.commands.push(command));
}

pub fn cancel(key: impl Hash) {
    let key = hash_of(key);
    effects("cancel", |effects| effects.jobs.push(Job::Cancel(key)));
}

pub fn spawn<T, M>(
    future: impl Future<Output = T> + Send + 'static,
    done: impl IntoHandler<(T,), (), M>,
) where
    T: Send + 'static,
{
    start(None, future_work(future), false, done.into_handler());
}

pub fn spawn_keyed<T, M>(
    key: impl Hash,
    future: impl Future<Output = T> + Send + 'static,
    done: impl IntoHandler<(T,), (), M>,
) where
    T: Send + 'static,
{
    start(
        Some(hash_of(key)),
        future_work(future),
        false,
        done.into_handler(),
    );
}

pub fn spawn_blocking<T, M>(
    work: impl FnOnce() -> T + Send + 'static,
    done: impl IntoHandler<(T,), (), M>,
) where
    T: Send + 'static,
{
    let work = Work::Blocking(Box::new(move || Box::new(work()) as Output));
    start(None, work, false, done.into_handler());
}

pub fn spawn_blocking_keyed<T, M>(
    key: impl Hash,
    work: impl FnOnce() -> T + Send + 'static,
    done: impl IntoHandler<(T,), (), M>,
) where
    T: Send + 'static,
{
    let work = Work::Blocking(Box::new(move || Box::new(work()) as Output));
    start(Some(hash_of(key)), work, false, done.into_handler());
}

pub fn stream<T, M>(
    stream: impl Stream<Item = T> + Send + 'static,
    each: impl IntoHandler<(T,), (), M>,
) where
    T: Send + 'static,
{
    start(None, stream_work(stream), true, each.into_handler());
}

pub fn stream_keyed<T, M>(
    key: impl Hash,
    stream: impl Stream<Item = T> + Send + 'static,
    each: impl IntoHandler<(T,), (), M>,
) where
    T: Send + 'static,
{
    start(
        Some(hash_of(key)),
        stream_work(stream),
        true,
        each.into_handler(),
    );
}

pub fn after<M>(delay: Duration, done: impl IntoHandler<(), (), M>) {
    let done = done.into_handler().map(|((),): ((),)| ());
    start(None, future_work(slept(delay)), false, done);
}

pub fn after_keyed<M>(key: impl Hash, delay: Duration, done: impl IntoHandler<(), (), M>) {
    let done = done.into_handler().map(|((),): ((),)| ());
    start(Some(hash_of(key)), future_work(slept(delay)), false, done);
}

pub fn every<M>(period: Duration, tick: impl IntoHandler<(), (), M>) {
    let tick = tick.into_handler().map(|((),): ((),)| ());
    start(None, stream_work(ticks(period)), true, tick);
}

pub fn every_keyed<M>(key: impl Hash, period: Duration, tick: impl IntoHandler<(), (), M>) {
    let tick = tick.into_handler().map(|((),): ((),)| ());
    start(Some(hash_of(key)), stream_work(ticks(period)), true, tick);
}

async fn slept(delay: Duration) {
    tokio::time::sleep(delay).await;
}

fn ticks(period: Duration) -> impl Stream<Item = ()> + Send + 'static {
    futures_util::stream::unfold((), move |()| async move {
        tokio::time::sleep(period).await;
        Some(((), ()))
    })
}

fn future_work<T: Send + 'static>(future: impl Future<Output = T> + Send + 'static) -> Work {
    Work::Future(Box::pin(future.map(|output| Box::new(output) as Output)))
}

fn stream_work<T: Send + 'static>(stream: impl Stream<Item = T> + Send + 'static) -> Work {
    Work::Stream(Box::pin(stream.map(|output| Box::new(output) as Output)))
}

fn start<T: 'static>(key: Option<u64>, work: Work, lasting: bool, done: Handler<(T,)>) {
    let id = JOBS.fetch_add(1, Ordering::Relaxed);
    let deliver: Deliver = Rc::new(move |reach, output| {
        let output = output.downcast::<T>().expect("job output type mismatch");
        done.call(reach, (*output,));
    });
    effects("spawn", |effects| {
        if let Some(key) = key {
            effects.jobs.push(Job::Cancel(key));
        }
        effects.jobs.push(Job::Start { id, key, work });
        let chain = effects.chain.clone();
        effects.pending.push((
            id,
            Pending {
                key,
                lasting,
                chain,
                deliver,
            },
        ));
    });
}
