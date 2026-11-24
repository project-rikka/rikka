mod arrange;
mod content;
mod dispatch;
mod dump;
mod graveyard;
mod handle;
mod holding;
mod images;
mod instance;
mod latch;
mod layout;
mod presses;
mod readers;
mod reconcile;
mod rendering;
mod route;
mod scene_sync;
mod scratch;
mod scroll_anchor;
mod table;
mod tipping;
mod tips;
mod tree;
mod typed;
mod verify;
mod visuals;
mod walk;
mod window_part;
mod zoom;

use std::rc::Rc;
use std::time::Instant;

use rikka_input::Command;
use rikka_types::{
    Color, CursorShape, Point, Rect, RoundedRect, Size, Stylus, WindowIdentifier, WindowView,
};
use rustc_hash::FxHashMap;

use crate::input::{Accept, DragStart, DragWindow, Event};
use crate::picture::DirectResolver;
use crate::runtime::tree::UiTree;

use crate::stats::Reason;
use crate::task::{Effects, IntoHandler, Job, JobResult, Pending, RuntimeMessage, Update};
use crate::view::observe::{log, reads};
use crate::view::{State, StateAccess, ViewFn};
use crate::window::{Identified, WindowPart, WindowRequest};
pub(crate) use handle::Marks;
pub use handle::{NodeHandle, Watchers};
pub(crate) use layout::Layouts;
pub use layout::{LayoutTree, Node as LayoutNode};
pub use rendering::Rendering;
use rikka_picture::Waking;
pub use typed::TypedUi;

use crate::picture::SourceKey;

use rikka_theme::Theme;

use content::{Content, RootState};

pub fn register_wake(wake: &Waking) {
    log::on_change(wake);
    rikka_picture::consume::on_arrival(wake);
    handle::on_marks(wake);
}

const PASSES: usize = 6;

pub struct Ui {
    content: Box<dyn Content>,
    tree: UiTree,
    needs_view: bool,
    everything: bool,
    translations: u64,
    reader: log::Reader,
    jobs: Vec<Job>,
    pending: FxHashMap<u64, Pending>,
    sent: Vec<Command>,
    identifier: Option<Rc<WindowIdentifier>>,
    _wake: Option<Waking>,
}

impl Ui {
    pub fn new<S: State>(state: S, view: impl for<'a> ViewFn<'a, S>, theme: Theme) -> Self {
        Self::holding(Box::new(RootState::new(state, view)), theme)
    }

    fn holding(content: Box<dyn Content>, theme: Theme) -> Self {
        Self {
            content,
            tree: UiTree::new(theme),
            needs_view: true,
            everything: true,
            translations: rikka_i18n::generation(),
            reader: log::Reader::new(),
            jobs: Vec::new(),
            pending: FxHashMap::default(),
            sent: Vec::new(),
            identifier: None,
            _wake: None,
        }
    }

    pub fn set_window_identifier(&mut self, identifier: Option<WindowIdentifier>) {
        self.identifier = identifier.map(Rc::new);
    }

    fn identified(&self) -> Identified {
        crate::window::identified(self.identifier.clone())
    }

    pub fn start<M>(&mut self, start: impl IntoHandler<(), (), M>) {
        let _identified = self.identified();
        let start = start.into_handler();
        let effects = self.acting(|reach| {
            start.call(reach, ());
        });
        self.absorb(effects);
    }

    pub fn send(&mut self, update: Update) {
        let _identified = self.identified();
        let effects = self.acting(|reach| {
            crate::task::acted();
            update.apply(reach);
        });
        self.absorb(effects);
    }

    pub fn receive(&mut self, inbound: RuntimeMessage) {
        match inbound {
            RuntimeMessage::JobResult(result) => self.complete_job(result),
            RuntimeMessage::Update(update) => self.send(update),
        }
    }

    pub fn complete_job(&mut self, result: JobResult) {
        let _identified = self.identified();
        let Some(pending) = self.pending.get(&result.job) else {
            return;
        };
        let deliver = std::rc::Rc::clone(&pending.deliver);
        let chain = pending.chain.clone();
        let lasting = pending.lasting;
        let Some(output) = result.output else {
            self.pending.remove(&result.job);
            return;
        };
        if !lasting {
            self.pending.remove(&result.job);
        }
        let ((), effects) = crate::task::collecting(|| {
            let mut reach = StateAccess::new(self.content.state(), &chain);
            deliver(&mut reach, output);
        });
        self.absorb(effects);
    }

    fn acting(&mut self, act: impl FnOnce(&mut StateAccess<'_>)) -> Effects {
        let ((), effects) = crate::task::collecting(|| {
            let mut reach = StateAccess::new(self.content.state(), &[]);
            act(&mut reach);
        });
        effects
    }

    fn absorb(&mut self, mut effects: Effects) {
        if effects.wants_view() {
            self.needs_view = true;
            self.everything = true;
        }
        if effects.frame {
            self.tree.repaint_everything();
        }
        if let Some(motion) = effects.animation {
            self.tree.transaction = Some(motion);
        }
        self.tree.requests.append(&mut effects.windows);
        self.sent.append(&mut effects.commands);
        let mut started: FxHashMap<u64, Pending> = effects.pending.drain(..).collect();
        for job in effects.jobs {
            match &job {
                Job::Cancel(key) => self.pending.retain(|_, pending| pending.key != Some(*key)),
                Job::Start { id, .. } => {
                    if let Some(pending) = started.remove(id) {
                        self.pending.insert(*id, pending);
                    }
                }
            }
            self.jobs.push(job);
        }
    }

    pub fn take_unclaimed_command(&mut self) -> Option<Command> {
        self.tree.unclaimed.take()
    }

    pub fn take_window_requests(&mut self) -> Vec<WindowRequest> {
        std::mem::take(&mut self.tree.requests)
    }

    pub fn take_drag_start(&mut self) -> Option<DragStart> {
        self.tree.drag_start.take()
    }

    pub fn drag_accepts(&self) -> Option<&Accept> {
        self.tree.drag_accept.as_ref()
    }

    pub fn take_drag_window(&mut self) -> Option<DragWindow> {
        self.tree.drag_window.take()
    }

    pub fn take_jobs(&mut self) -> Vec<Job> {
        std::mem::take(&mut self.jobs)
    }

    pub fn cursor(&self) -> CursorShape {
        self.tree.cursor_shape
    }

    pub fn pointer_locked(&self) -> bool {
        self.tree.locked
    }

    #[doc(hidden)]
    pub fn dump(&self) -> String {
        self.tree.dump()
    }

    pub fn content_size(&self) -> Size {
        self.tree.content_size()
    }

    pub fn set_measured(&mut self, measured: bool) {
        self.tree.set_measured(measured);
    }

    pub fn caret(&self) -> Option<Rect> {
        self.tree.caret()
    }

    pub fn grabs_keys(&self) -> bool {
        self.tree.grabs_keys()
    }

    pub fn window_part_at(&self, point: Point) -> Option<WindowPart> {
        self.tree.window_part_at(point)
    }

    pub fn opaque(&self) -> &[RoundedRect] {
        self.tree.opaque()
    }

    pub fn glass(&self) -> &[RoundedRect] {
        self.tree.glass()
    }

    pub fn windows(&self) -> &[WindowView] {
        self.tree.windows()
    }

    pub fn take_pulses(&mut self, into: &mut Vec<rikka_picture::Pulse>) {
        self.tree.take_pulses(into);
    }

    pub fn shows_image(&self, key: &SourceKey) -> bool {
        self.tree.shows_image(key)
    }

    pub fn handle_event(&mut self, event: Event) -> bool {
        self.handle_input(event, None)
    }

    pub fn handle_input(&mut self, event: Event, stylus: Option<Stylus>) -> bool {
        let _identified = self.identified();
        let (handled, effects) = crate::task::collecting(|| {
            self.tree
                .dispatch(&event, stylus.as_ref(), self.content.state())
        });
        self.absorb(effects);
        handled
    }

    pub fn set_viewport(&mut self, size: Size, scale_factor: f32) {
        self.tree.set_viewport(size, scale_factor);
    }

    pub fn set_theme(&mut self, theme: Theme) {
        if self.tree.set_theme(theme) {
            self.needs_view = true;
        }
    }

    pub fn theme(&self) -> &Theme {
        &self.tree.theme
    }

    pub fn scale_factor(&self) -> f32 {
        self.tree.scale
    }

    pub fn arrive(&mut self) {
        self.tree.arrive();
    }

    pub fn leave(&mut self) {
        self.set_leaving(true);
    }

    pub fn rejoin(&mut self) {
        self.set_leaving(false);
    }

    fn set_leaving(&mut self, leaving: bool) {
        if self.tree.set_leaving(leaving) {
            self.needs_view = true;
            self.everything = true;
        }
    }

    pub fn is_leaving(&self) -> bool {
        self.tree.is_leaving()
    }

    pub fn set_background(&mut self, background: Option<Color>) {
        if self.tree.background != background {
            self.tree.background = background;
            self.tree.needs_frame = true;
        }
    }

    pub fn needs_render(&self) -> bool {
        self.has_pending_work()
            || !self.sent.is_empty()
            || self.tree.animating
            || self.woken(Instant::now())
            || self.translations_changed()
    }

    pub fn wakes_with(&mut self, wake: Waking) {
        register_wake(&wake);
        self._wake = Some(wake);
    }

    pub fn next_wake(&self) -> Option<Instant> {
        self.tree.next_wake()
    }

    pub fn wants_prefetch(&self) -> bool {
        self.tree.wants_prefetch()
    }

    pub fn prefetch(&mut self, scene: &mut rikka_renderer::Scene, until: Instant) {
        if !self.tree.wants_prefetch() || self.has_pending_work() || !self.sent.is_empty() {
            return;
        }
        let _identified = self.identified();
        let Self { tree, content, .. } = self;
        let ((), effects) =
            crate::task::collecting(|| tree.prefetch(scene, content.state(), until));
        self.absorb(effects);
    }

    fn woken(&self, now: Instant) -> bool {
        self.tree.next_wake().is_some_and(|at| at <= now)
    }

    fn translations_changed(&self) -> bool {
        self.translations != rikka_i18n::generation()
    }

    fn has_pending_work(&self) -> bool {
        self.needs_view
            || self.tree.needs_frame
            || self.tree.wants_view
            || self.tree.wants_build
            || self.tree.marks.pending()
            || self.reader.pending(|key| self.tree.reads(key))
    }

    fn take_changes(&mut self) {
        let changed = self.tree.take_changes(self.reader.take());
        self.needs_view |= changed.root || changed.everything;
        self.everything |= changed.everything;
    }

    pub fn request_frame(&mut self) {
        self.tree.repaint_everything();
    }

    pub fn request_redraw(&mut self) {
        self.tree.needs_frame = true;
    }

    pub fn images_arrived(&mut self, settled: &[SourceKey], updated: bool) -> bool {
        self.tree.images_arrived(settled, updated)
    }

    pub fn shown_images(&self) -> impl Iterator<Item = &SourceKey> {
        self.tree.shown_images()
    }

    pub fn render<'a>(&mut self, rendering: impl Into<Rendering<'a>>) {
        let _identified = self.identified();
        let rendering = rendering.into();
        let clock = rendering.clock();
        let Rendering {
            scene, resources, ..
        } = rendering;
        let now = clock.now;
        let mut direct = DirectResolver;
        let resources = resources.unwrap_or(&mut direct);
        let _span = tracing::trace_span!("rikka_ui::render").entered();
        let mut cause = match (self.tree.animating, self.woken(now)) {
            (true, _) => Some(Reason::Animation),
            (false, true) => Some(Reason::Wake),
            (false, false) => None,
        };
        if cause.is_some() {
            self.tree.wake = None;
            self.tree.needs_frame = true;
        }
        if self.translations_changed() {
            self.translations = rikka_i18n::generation();
            log::languages(self.translations);
        }
        for _ in 0..PASSES {
            crate::stats::passed();
            self.tree.take_marks();
            self.take_changes();
            let mut reason = None;
            if self.needs_view || self.tree.wants_view {
                let everything = std::mem::take(&mut self.everything) || self.tree.wants_view;
                self.needs_view = false;
                self.tree.wants_view = false;
                reason = Some(Reason::View);
                if let Some(theme) = self.content.theme() {
                    self.tree.set_theme(theme);
                }
                let theme = self.tree.theme.clone();
                let env = self.tree.env();
                let built = log::now();
                let ((element, reads), effects) = crate::task::collecting(|| {
                    let _span = tracing::trace_span!("rikka_ui::view").entered();
                    crate::stats::viewed();
                    reads::recorded(|| {
                        crate::ambient::with_env(&theme, env, || self.content.view())
                    })
                });
                self.absorb(effects);
                self.tree.read_at_root(reads, built);
                let ((), effects) = crate::task::collecting(|| {
                    self.tree
                        .set_root(element, scene, self.content.state(), everything);
                });
                self.absorb(effects);
            } else if self.tree.wants_build {
                reason = Some(Reason::Build);
                let ((), effects) = crate::task::collecting(|| {
                    self.tree.rebuild_asked(scene, self.content.state())
                });
                self.absorb(effects);
            }
            if self.tree.needs_frame {
                crate::stats::framed(reason.or(cause.take()).unwrap_or(Reason::Repaint));
                self.tree
                    .sync(scene, clock, resources, self.content.state());
                let ((), effects) = crate::task::collecting(|| {
                    self.tree.refresh_hover(self.content.state());
                    self.tree.refresh_focus(self.content.state());
                });
                self.absorb(effects);
            }
            if !self.has_pending_work() && !self.deliver_sent() {
                break;
            }
        }
    }

    fn deliver_sent(&mut self) -> bool {
        if self.sent.is_empty() {
            return false;
        }
        for command in std::mem::take(&mut self.sent) {
            self.handle_event(Event::Command(command));
        }
        true
    }
}

impl std::fmt::Debug for Ui {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Ui")
            .field("state", &self.content.name())
            .field("needs_view", &self.needs_view)
            .field("pending", &self.pending.len())
            .finish_non_exhaustive()
    }
}
