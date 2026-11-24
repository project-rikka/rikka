use std::sync::Arc;
use std::time::Instant;

use rikka_animation::Motion;
use rikka_input::Command;
use rikka_renderer::Scene;
use rikka_theme::Theme;
use rikka_types::{Color, CursorShape, Point, Rect, RoundedRect, Size, WindowView};
use rustc_hash::{FxHashMap, FxHashSet};
use smallvec::SmallVec;

use crate::ambient::ThemeHandle;
use crate::input::{Accept, DragStart, DragWindow};
use crate::motion::Clock;
use crate::picture::SourceKey;
use crate::picture::TextureResolver;
use crate::runtime::arrange::AnchorPass;
use crate::runtime::graveyard::Graveyard;
use crate::runtime::handle::{Mark, Marks};
use crate::runtime::images::Images;
use crate::runtime::latch::Latch;
use crate::runtime::layout::{Layouts, Node};
use crate::runtime::presses::Presses;
use crate::runtime::readers::{Producer, Readers};
use crate::runtime::reconcile::{
    ReconcileCx, create, current_element, last_of, rebuild_walk, reconcile, remeasure, restyle,
    teardown,
};
use crate::runtime::route::HitRoute;
use crate::runtime::scene_sync::{Sink, refresh_images};
use crate::runtime::scroll_anchor::ScrollAnchors;
use crate::runtime::table::{CULLED, DissolvedIsland, PAINT, Table, VISIT};
use crate::runtime::tips::Tips;
use crate::runtime::walk::{Spare, Walk, WalkResult};
use crate::view::observe::log::{self, LANGUAGE, LogSlice};
use crate::view::{Behavior, Element, Env, State};
use crate::window::WindowRequest;
use rikka_picture::Pulse;

pub(crate) struct UiTree {
    pub(super) layouts: Layouts,
    pub(crate) table: Table,
    pub(crate) root: Option<Node>,
    pub(crate) theme: ThemeHandle,
    pub(crate) viewport: Size,
    pub(crate) scale: f32,
    pub(super) measured: bool,
    epoch: u64,
    leaving: bool,
    pub(crate) cursor: Option<Point>,
    pub(crate) cursor_shape: CursorShape,
    pub(crate) hovered: HitRoute,
    pub(crate) probe: HitRoute,
    pub(crate) capture: Option<u64>,
    pub(crate) exclusive: bool,
    pub(crate) locked: bool,
    pub(crate) presses: Presses,
    pub(crate) focus: Option<u64>,
    pub(crate) focus_before: Option<u64>,
    pub(crate) pressed: Vec<u64>,
    pub(crate) unclaimed: Option<Command>,
    pub(crate) notified_focus: Option<u64>,
    pub(crate) needs_frame: bool,
    pub(super) resized: bool,
    pub(crate) animating: bool,
    pub(crate) wants_view: bool,
    pub(crate) wants_build: bool,
    pub(crate) requests: Vec<WindowRequest>,
    pub(crate) drag_start: Option<DragStart>,
    pub(crate) drag_origin: Option<u64>,
    pub(crate) drag_target: Option<u64>,
    pub(crate) drag_accept: Option<Accept>,
    pub(crate) drag_window: Option<DragWindow>,
    pub(crate) background: Option<Color>,
    background_synced: Option<Color>,
    glass: Vec<RoundedRect>,
    opaque: Vec<RoundedRect>,
    windows: Vec<WindowView>,
    glass_synced: Vec<RoundedRect>,
    windows_synced: Vec<WindowView>,
    pub(super) has_anchors: bool,
    autofocus: Vec<u64>,
    claimed: Option<u64>,
    pub(crate) modals: Vec<u64>,
    top_modal: Option<u64>,
    parked_focus: Option<u64>,
    pub(super) anchors: AnchorPass,
    pub(super) scroll_anchors: ScrollAnchors,
    pub(super) latch: Latch,
    pub(crate) transaction: Option<Motion>,
    arrivals: bool,
    debut: Option<Instant>,
    pub(crate) wake: Option<Instant>,
    pub(crate) tips: Tips,
    pub(crate) readers: Readers,
    root_reads: Vec<u64>,
    root_built: u64,
    theme_key: u64,
    taken_at: u64,
    pub(super) touched: bool,
    pub(super) hit_stale: bool,
    pub(crate) marks: Arc<Marks>,
    pub(crate) images: Images,
    pub(super) scrollers: FxHashSet<Node>,
    pub(super) arrangers: FxHashSet<Node>,
    hosting: FxHashSet<Node>,
    imaged: FxHashSet<Node>,
    active: Vec<u64>,
    wakes: FxHashMap<u64, Instant>,
    repaint_all: bool,
    revisit_all: bool,
    caret: Option<Rect>,
    grabs: bool,
    spare: Spare,
    walked: WalkResult,
    prefetches: Vec<u64>,
    pulses: Vec<Pulse>,
    hosting_order: Vec<Node>,
    moved: Vec<Node>,
    graveyard: Graveyard,
    pub(super) arranging: Vec<Node>,
    pub(super) arranged_rects: Vec<Rect>,
}

impl UiTree {
    pub(crate) fn new(theme: Theme) -> Self {
        let theme = ThemeHandle::new(theme);
        Self {
            layouts: Layouts::default(),
            table: Table::default(),
            root: None,
            theme,
            viewport: Size::ZERO,
            scale: 1.0,
            leaving: false,
            measured: false,
            epoch: 0,
            cursor: None,
            cursor_shape: CursorShape::default(),
            hovered: HitRoute::default(),
            probe: HitRoute::default(),
            capture: None,
            exclusive: false,
            locked: false,
            presses: Presses::default(),
            focus: None,
            pressed: Vec::new(),
            unclaimed: None,
            focus_before: None,
            notified_focus: None,
            needs_frame: true,
            resized: true,
            animating: false,
            wants_view: false,
            wants_build: false,
            requests: Vec::new(),
            drag_start: None,
            drag_origin: None,
            drag_target: None,
            drag_accept: None,
            drag_window: None,
            background: None,
            background_synced: None,
            glass: Vec::new(),
            opaque: Vec::new(),
            windows: Vec::new(),
            glass_synced: Vec::new(),
            windows_synced: Vec::new(),
            has_anchors: false,
            autofocus: Vec::new(),
            claimed: None,
            modals: Vec::new(),
            top_modal: None,
            parked_focus: None,
            anchors: AnchorPass::default(),
            scroll_anchors: ScrollAnchors::default(),
            latch: Latch::default(),
            transaction: None,
            arrivals: false,
            debut: None,
            wake: None,
            tips: Tips::default(),
            readers: Readers::default(),
            root_reads: Vec::new(),
            root_built: 0,
            theme_key: log::key(),
            taken_at: 0,
            touched: true,
            hit_stale: true,
            marks: Arc::default(),
            images: Images::default(),
            scrollers: FxHashSet::default(),
            arrangers: FxHashSet::default(),
            hosting: FxHashSet::default(),
            imaged: FxHashSet::default(),
            active: Vec::new(),
            wakes: FxHashMap::default(),
            repaint_all: false,
            revisit_all: false,
            spare: Spare::default(),
            walked: WalkResult::default(),
            prefetches: Vec::new(),
            pulses: Vec::new(),
            hosting_order: Vec::new(),
            moved: Vec::new(),
            graveyard: Graveyard::default(),
            arranging: Vec::new(),
            arranged_rects: Vec::new(),
            caret: None,
            grabs: false,
        }
    }

    pub(crate) fn set_viewport(&mut self, size: Size, scale: f32) {
        if self.viewport == size && self.scale == scale {
            return;
        }
        self.viewport = size;
        self.scale = scale;
        self.resized = true;
        self.revisit_all = true;
        self.needs_frame = true;
    }

    pub(crate) fn set_measured(&mut self, measured: bool) {
        if self.measured == measured {
            return;
        }
        self.measured = measured;
        self.resized = true;
        self.needs_frame = true;
    }

    pub(crate) fn set_theme(&mut self, theme: Theme) -> bool {
        if *self.theme == theme {
            return false;
        }
        self.theme = ThemeHandle::new(theme);
        self.retire();
        log::changed(self.theme_key);
        self.needs_frame = true;
        true
    }

    pub(crate) fn set_leaving(&mut self, leaving: bool) -> bool {
        if self.leaving == leaving {
            return false;
        }
        self.leaving = leaving;
        if leaving {
            self.tips.leave();
        }
        self.retire();
        self.needs_frame = true;
        true
    }

    pub(crate) fn is_leaving(&self) -> bool {
        self.leaving
    }

    pub(crate) fn arrive(&mut self) {
        self.arrivals = true;
    }

    pub(crate) fn env(&self) -> Env {
        Env {
            leaving: self.leaving,
            ..Env::from_theme(&self.theme)
        }
    }

    pub(crate) fn retire(&mut self) {
        self.epoch += 1;
        self.repaint_all = true;
    }

    pub(crate) fn repaint_everything(&mut self) {
        self.repaint_all = true;
        self.needs_frame = true;
    }

    fn reconciling<R>(
        &mut self,
        scene: &mut Scene,
        state: &mut dyn State,
        arrivals: bool,
        everything: bool,
        body: impl FnOnce(&mut ReconcileCx<'_>, Option<Node>) -> R,
    ) -> (R, bool) {
        let root = self.root;
        let (result, attributed, retained) = {
            let mut cx = ReconcileCx {
                layouts: &mut self.layouts,
                table: &mut self.table,
                scene,
                images: &mut self.images,
                marks: &self.marks,
                scrollers: &mut self.scrollers,
                arrangers: &mut self.arrangers,
                hosting: &mut self.hosting,
                imaged: &mut self.imaged,
                theme: &self.theme,
                epoch: self.epoch,
                has_anchors: &mut self.has_anchors,
                autofocus: &mut self.autofocus,
                claimed: &mut self.claimed,
                modals: &mut self.modals,
                arrivals,
                state,
                readers: &mut self.readers,
                chain: Vec::new(),
                producer: Producer::Root,
                attributed: Vec::new(),
                everything,
                retained: false,
                taken_at: self.taken_at,
                graveyard: &mut self.graveyard,
            };
            let result = body(&mut cx, root);
            (result, cx.attributed, cx.retained)
        };
        self.graveyard.bury_overflow();
        self.refresh_islands(scene);
        self.attribute(attributed);
        (result, retained)
    }

    fn attribute(&mut self, attributed: Vec<(Producer, Vec<u64>)>) {
        for (producer, keys) in attributed {
            let reads = match producer {
                Producer::Root => &mut self.root_reads,
                Producer::View(id) => {
                    let Some(component) = self
                        .table
                        .find(id)
                        .and_then(|node| self.table.get_mut(node).component.as_mut())
                    else {
                        continue;
                    };
                    &mut component.reads
                }
                Producer::Build(id) => {
                    let Some(node) = self.table.find(id) else {
                        continue;
                    };
                    &mut self.table.get_mut(node).build_reads
                }
            };
            reads.extend_from_slice(&keys);
            reads.sort_unstable();
            reads.dedup();
            self.readers.add(producer, &keys);
        }
    }

    pub(crate) fn read_at_root(&mut self, reads: Vec<u64>, built: u64) {
        self.readers.remove(Producer::Root, &self.root_reads);
        self.readers.add(Producer::Root, &reads);
        self.root_reads = reads;
        self.root_built = built;
    }

    pub(crate) fn set_root(
        &mut self,
        element: Element,
        scene: &mut Scene,
        state: &mut dyn State,
        everything: bool,
    ) {
        let _span = tracing::trace_span!("rikka_ui::reconcile").entered();
        let env = self.env();
        self.has_anchors = false;
        self.autofocus.clear();
        self.claimed = None;
        self.modals.clear();
        self.wants_build = false;
        let arrivals = self.arrivals;
        let (root, retained) =
            self.reconciling(scene, state, arrivals, everything, |cx, root| match root {
                Some(root) if cx.table.get(root).identity() == element.identity() => {
                    let now = reconcile(cx, root, element, &env);
                    if now != root {
                        settle_root(cx, now);
                    }
                    now
                }
                old => {
                    if let Some(old) = old {
                        teardown(cx, old);
                    }
                    let root = create(cx, element, &env);
                    settle_root(cx, root);
                    root
                }
            });
        self.table.relabel(&self.layouts, scene);
        self.arrivals = true;
        self.root = Some(root);
        if retained {
            self.recollect();
        }
        self.reshaped();
        self.settle_focus();
        self.needs_frame = true;
    }

    fn reshaped(&mut self) {
        self.touched = true;
        self.hit_stale = true;
    }

    pub(crate) fn wants_prefetch(&self) -> bool {
        !self.prefetches.is_empty() || !self.graveyard.is_empty()
    }

    pub(crate) fn prefetch(&mut self, scene: &mut Scene, state: &mut dyn State, until: Instant) {
        self.graveyard.bury(until);
        if Instant::now() >= until {
            return;
        }
        for id in std::mem::take(&mut self.prefetches) {
            if let Some(node) = self.table.find(id) {
                self.table.get_mut(node).asked_build = true;
                self.wants_build_above(node);
            }
        }
        if !self.wants_build {
            return;
        }
        crate::ambient::prefetching_until(until, || {
            self.rebuild_asked(scene, state);
            self.arrange(scene, state);
        });
    }

    pub(crate) fn rebuild_asked(&mut self, scene: &mut Scene, state: &mut dyn State) {
        let _span = tracing::trace_span!("rikka_ui::rebuild").entered();
        self.wants_build = false;
        self.claimed = None;
        let ((singular, root), retained) =
            self.reconciling(scene, state, true, false, |cx, root| {
                let Some(root) = root else {
                    return (false, None);
                };
                match current_element(cx, root) {
                    Some((element, env)) => {
                        let now = reconcile(cx, root, element, &env);
                        if now != root {
                            settle_root(cx, now);
                        }
                        (true, Some(now))
                    }
                    None => (rebuild_walk(cx, root), Some(root)),
                }
            });
        self.root = root;
        self.table.relabel(&self.layouts, scene);
        if singular || retained {
            self.recollect();
        }
        self.reshaped();
        self.settle_focus();
        self.needs_frame = true;
    }

    pub(crate) fn take_changes(&mut self, slice: LogSlice) -> Invalidation {
        self.taken_at = slice.at;
        let mut changed = Invalidation {
            root: false,
            everything: slice.overrun,
        };
        let mut universal = false;
        for (stamp, key) in slice.changes {
            if key == LANGUAGE || key == self.theme_key {
                universal |= stamp > self.root_built;
                continue;
            }
            let producers: SmallVec<[Producer; 4]> = SmallVec::from(self.readers.producers(key));
            for producer in producers {
                match producer {
                    Producer::Root => changed.root |= stamp > self.root_built,
                    Producer::View(id) => self.stale_view(id, stamp),
                    Producer::Build(id) => self.stale_build(id, stamp),
                }
            }
        }
        if universal {
            self.retire();
            changed.root = true;
        }
        changed
    }

    fn stale_view(&mut self, id: u64, stamp: u64) {
        let Some(node) = self.table.find(id) else {
            return;
        };
        let Some(component) = self.table.get_mut(node).component.as_mut() else {
            return;
        };
        if component.built < stamp && !component.dirty {
            component.dirty = true;
            self.wants_build_above(node);
        }
    }

    fn stale_build(&mut self, id: u64, stamp: u64) {
        let Some(node) = self.table.find(id) else {
            return;
        };
        let instance = self.table.get_mut(node);
        if instance.build_stamp < stamp && !instance.asked_build {
            instance.asked_build = true;
            self.wants_build_above(node);
        }
    }

    pub(crate) fn reads(&self, key: u64) -> bool {
        key == LANGUAGE || key == self.theme_key || self.readers.reads(key)
    }

    pub(crate) fn take_marks(&mut self) {
        for (id, mark) in self.marks.take() {
            let Some(node) = self.table.find(id) else {
                continue;
            };
            match mark {
                Mark::Repaint => self.table.mark(&self.layouts, node, PAINT),
                Mark::Rebuild => {
                    self.table.get_mut(node).asked_build = true;
                    self.wants_build_above(node);
                }
                Mark::Relayout => self.relayout(node),
            }
            self.needs_frame = true;
        }
    }

    fn relayout(&mut self, node: Node) {
        let mut instance = self.table.take(node);
        restyle(
            &mut self.layouts,
            &mut self.scrollers,
            &mut self.table,
            &mut instance,
            &self.theme,
        );
        remeasure(&mut self.layouts, &self.theme, &instance);
        self.table.put(node, instance);
        self.layouts.refit(node);
        self.table.mark(&self.layouts, node, PAINT);
    }

    pub(super) fn wants_build_above(&mut self, node: Node) {
        let mut at = Some(node);
        while let Some(node) = at {
            self.table.get_mut(node).wants_build = true;
            at = self.layouts.parent(node);
        }
        self.wants_build = true;
    }

    fn recollect(&mut self) {
        self.has_anchors = false;
        self.autofocus.clear();
        self.modals.clear();
        if let Some(root) = self.root {
            self.collect_singular(root, false);
        }
    }

    fn collect_singular(&mut self, node: Node, inert: bool) {
        let instance = self.table.get(node);
        if !instance.singular {
            return;
        }
        let behavior = instance.behavior;
        self.has_anchors |= instance.anchor.is_some();
        let inert = inert || behavior.has(Behavior::INERT);
        if !inert && behavior.has(Behavior::MODAL) {
            self.modals.push(instance.id);
        }
        if !inert && behavior.has(Behavior::AUTOFOCUS) && behavior.focusable() {
            self.autofocus.push(instance.id);
        }
        for index in 0..self.layouts.children(node).len() {
            let child = self.layouts.children(node)[index];
            self.collect_singular(child, inert);
        }
    }

    pub(super) fn unlock(&mut self) {
        if std::mem::take(&mut self.locked) {
            self.cursor_shape = CursorShape::Default;
            self.hit_stale = true;
        }
    }

    fn settle_focus(&mut self) {
        if let Some(id) = self.capture
            && self.live(id).is_none()
        {
            self.capture = None;
            self.exclusive = false;
            self.unlock();
        }
        let before = self.focus;
        if let Some(id) = self
            .claimed
            .filter(|id| self.within_top_modal(*id) && self.focusable(*id))
        {
            self.set_focused(Some(id));
        }
        if let Some(id) = self.focus
            && !self.focusable(id)
        {
            self.set_focused(None);
        }
        self.hand_over_focus(before);
        if self.focus.is_none() {
            let autofocus = self
                .autofocus
                .iter()
                .copied()
                .find(|id| self.focusable(*id) && self.within_top_modal(*id));
            self.set_focused(autofocus);
        }
    }

    pub(super) fn top_modal_node(&self) -> Option<Node> {
        self.modals.last().and_then(|modal| self.table.find(*modal))
    }

    pub(super) fn within_top_modal(&self, id: u64) -> bool {
        let Some(modal) = self.top_modal_node() else {
            return true;
        };
        let Some(node) = self.table.find(id) else {
            return false;
        };
        let mut at = Some(node);
        while let Some(step) = at {
            if step == modal {
                return true;
            }
            at = self.layouts.parent(step);
        }
        false
    }

    pub(super) fn set_focused(&mut self, focus: Option<u64>) {
        if self.focus == focus {
            return;
        }
        for id in [self.focus, focus].into_iter().flatten() {
            if let Some(node) = self.table.find(id) {
                self.table.mark(&self.layouts, node, PAINT);
            }
        }
        self.focus = focus;
    }

    fn hand_over_focus(&mut self, before: Option<u64>) {
        let top_modal = self.modals.last().copied();
        if self.top_modal == top_modal {
            return;
        }
        if self.top_modal.is_none() {
            self.parked_focus = before;
        }
        self.top_modal = top_modal;
        if let Some(id) = top_modal {
            let inside = self.focus.is_some_and(|focus| self.within_top_modal(focus));
            if !inside {
                self.set_focused(self.focusable(id).then_some(id));
            }
        } else if self.focus.is_none() {
            let parked = self.parked_focus.take().filter(|id| self.focusable(*id));
            self.set_focused(parked);
        } else {
            self.parked_focus = None;
        }
    }

    pub(super) fn unpark_focus(&mut self, id: u64, back: Option<u64>) {
        if self.parked_focus == Some(id) {
            self.parked_focus = back;
        }
    }

    pub(super) fn has_anchors(&self) -> bool {
        self.has_anchors
    }

    pub(super) fn focusable(&self, id: u64) -> bool {
        self.live(id)
            .is_some_and(|node| self.table.behavior(node).focusable())
    }

    pub(super) fn live(&self, id: u64) -> Option<Node> {
        let node = self.table.find(id)?;
        let mut at = Some(node);
        while let Some(step) = at {
            if self.table.behavior(step).has(Behavior::INERT) {
                return None;
            }
            at = self.layouts.parent(step);
        }
        Some(node)
    }

    pub(super) fn path_to(&self, id: u64) -> Option<Vec<Node>> {
        let node = self.table.find(id)?;
        let mut path = Vec::new();
        self.layouts.lineage(node, &mut path);
        Some(path)
    }

    pub(super) fn focus_takes_tab(&self) -> bool {
        let Some(node) = self.focus.and_then(|id| self.table.find(id)) else {
            return false;
        };
        let mut at = Some(node);
        while let Some(step) = at {
            if self.table.behavior(step).has(Behavior::TAKES_TAB) {
                return true;
            }
            at = self.layouts.parent(step);
        }
        false
    }

    pub(crate) fn content_size(&self) -> Size {
        let Some(root) = self.root else {
            return Size::ZERO;
        };
        self.layouts.rect(root).size
    }

    pub(crate) fn opaque(&self) -> &[RoundedRect] {
        &self.opaque
    }

    pub(crate) fn glass(&self) -> &[RoundedRect] {
        &self.glass
    }

    pub(crate) fn windows(&self) -> &[WindowView] {
        &self.windows
    }

    pub(crate) fn take_pulses(&mut self, into: &mut Vec<Pulse>) {
        for pulse in self.pulses.drain(..) {
            Pulse::join(into, pulse);
        }
    }

    pub(crate) fn caret(&self) -> Option<Rect> {
        self.caret
    }

    pub(crate) fn grabs_keys(&self) -> bool {
        self.grabs
    }

    pub(super) fn settle_caret(&mut self) {
        let focused = self.focus.and_then(|id| self.live(id));
        self.caret = focused.and_then(|node| {
            let instance = self.table.get(node);
            let within =
                instance
                    .widget
                    .caret(instance.state.as_ref(), &self.theme, &instance.env)?;
            Some(within.translate(self.table.bounds(&self.layouts, node).origin))
        });
        self.grabs =
            focused.is_some_and(|node| self.table.behavior(node).has(Behavior::GRABS_KEYS));
    }

    pub(crate) fn shown_images(&self) -> impl Iterator<Item = &SourceKey> {
        self.images.shown()
    }

    pub(crate) fn shows_image(&self, key: &SourceKey) -> bool {
        self.images.shows(key)
    }

    pub(crate) fn images_arrived(&mut self, settled: &[SourceKey], updated: bool) -> bool {
        if self.images.is_empty() {
            return false;
        }
        let waited = self.images.settled(settled);
        if updated {
            self.images.refreshed();
        }
        let frame_needed = waited || updated;
        self.needs_frame |= frame_needed;
        frame_needed
    }

    pub(crate) fn sync(
        &mut self,
        scene: &mut Scene,
        clock: Clock,
        resources: &mut dyn TextureResolver,
        state: &mut dyn State,
    ) {
        let _span = tracing::trace_span!("rikka_ui::sync").entered();
        self.needs_frame = false;
        self.animating = false;
        self.glass.clear();
        self.opaque.clear();
        self.windows.clear();

        let background = self.background.unwrap_or(self.theme.palette().background);
        if self.background_synced != Some(background) {
            scene.set_background(background);
            self.background_synced = Some(background);
        }

        if self.root.is_none() {
            return;
        }
        self.wake_due(clock.now);
        self.arrange(scene, state);
        let mut moved = std::mem::take(&mut self.moved);
        self.layouts.take_moved(&mut moved);
        for &node in &moved {
            if self.table.holds(node) {
                self.table.mark(&self.layouts, node, VISIT);
            }
        }
        self.moved = moved;
        self.refresh_islands(scene);
        if self.images.take_stale() {
            self.refresh_images(scene, resources);
        }
        let Some(root) = self.root else {
            return;
        };
        self.settle_tips(root, clock.now);
        let bubble = self.tips.painted().map(|bubble| (root, bubble));
        let env = self.env();
        let mut walked = {
            let _span = tracing::trace_span!("rikka_ui::paint").entered();
            let mut out = std::mem::take(&mut self.walked);
            out.reset();
            let mut walk = Walk {
                layouts: &self.layouts,
                table: &mut self.table,
                sink: Sink {
                    scene,
                    resources,
                    images: &mut self.images,
                },
                theme: &self.theme,
                marks: &self.marks,
                viewport: self.viewport,
                focus: self.focus,
                clock,
                debut: *self.debut.get_or_insert(clock.now),
                transaction: self.transaction.take(),
                pivots: &self.anchors.pivots,
                all: std::mem::take(&mut self.repaint_all),
                spare: std::mem::take(&mut self.spare),
                reveal: None,
                bubble,
                out,
            };
            let force = std::mem::take(&mut self.revisit_all);
            let theme = walk.theme;
            crate::ambient::with_env(theme, env, || walk.run(self.scale, force));
            self.spare = walk.spare;
            walk.out
        };
        self.settle_walked(&mut walked, root, scene);
        self.walked = walked;
        if self.settle_tips(root, clock.now) {
            self.needs_frame = true;
        }
        self.collect_hosted(scene, background);
        self.settle_caret();
    }

    fn wake_due(&mut self, now: Instant) {
        for &id in &self.active {
            if let Some(node) = self.table.find(id) {
                self.table.mark(&self.layouts, node, PAINT);
            }
        }
        self.active.clear();
        let (table, layouts) = (&mut self.table, &self.layouts);
        self.wakes.retain(|&id, &mut at| {
            if at > now {
                return true;
            }
            if let Some(node) = table.find(id) {
                table.mark(layouts, node, PAINT);
            }
            false
        });
    }

    fn refresh_islands(&mut self, scene: &mut Scene) {
        let mut islands = std::mem::take(&mut self.table.islands);
        if islands.is_stale() {
            let table = &self.table;
            islands.refresh(&self.layouts, |node| table.get(node).bound());
        }
        let dissolved = islands.take_dissolved();
        self.table.islands = islands;
        for DissolvedIsland { group, owner } in dissolved {
            scene.remove_group(group);
            if let Some(owner) = owner.filter(|owner| self.table.holds(*owner)) {
                self.table.mark(&self.layouts, owner, VISIT);
            }
        }
    }

    fn refresh_images(&mut self, scene: &mut Scene, resources: &mut dyn TextureResolver) {
        let nodes: Vec<Node> = self.imaged.iter().copied().collect();
        let mut sink = Sink {
            scene,
            resources,
            images: &mut self.images,
        };
        for node in nodes {
            if !self.table.holds(node) {
                continue;
            }
            let instance = self.table.get_mut(node);
            if instance.prims.iter().any(|slot| slot.image.is_some()) {
                refresh_images(&mut sink, &mut instance.prims);
            }
        }
    }

    fn settle_walked(&mut self, walked: &mut WalkResult, root: Node, scene: &mut Scene) {
        std::mem::swap(&mut self.active, &mut walked.active);
        self.wakes
            .extend(walked.wakes.drain(..).map(|(at, id)| (id, at)));
        self.wake = self.wakes.values().min().copied();
        self.animating = !self.active.is_empty();
        self.wants_view = walked.view;
        for node in walked.builds.drain(..) {
            if self.table.holds(node) {
                self.wants_build_above(node);
            }
        }
        for id in walked.prefetches.drain(..) {
            if !self.prefetches.contains(&id) {
                self.prefetches.push(id);
            }
        }
        for pulse in walked.pulses.drain(..) {
            Pulse::join(&mut self.pulses, pulse);
        }
        self.wants_build = self.table.get(root).wants_build;
        self.hit_stale |= walked.moved;
        for node in walked.shifted.drain(..) {
            self.table.islands.mark_around(node, &self.layouts);
        }
        for (node, imaged) in walked.imaged.drain(..) {
            match imaged {
                true => self.imaged.insert(node),
                false => self.imaged.remove(&node),
            };
        }
        for (node, hosts) in walked.hosting.drain(..) {
            match hosts {
                true => self.hosting.insert(node),
                false => self.hosting.remove(&node),
            };
        }
        if !walked.departed.is_empty() {
            let departed = &mut walked.departed;
            self.reconciling(scene, &mut (), true, false, |cx, _| {
                for node in departed.drain(..) {
                    if cx.table.holds(node) {
                        teardown(cx, node);
                    }
                }
            });
            self.reshaped();
            self.needs_frame = true;
        }
    }

    fn collect_hosted(&mut self, scene: &mut Scene, background: Color) {
        let mut hosting = std::mem::take(&mut self.hosting_order);
        hosting.clear();
        hosting.extend(
            self.hosting
                .iter()
                .copied()
                .filter(|node| self.table.holds(*node) && !self.table.has(*node, CULLED)),
        );
        hosting.sort_by_key(|node| self.table.labels.label(*node));
        let mut grounded = false;
        for &node in &hosting {
            let instance = self.table.get(node);
            let outputs = &instance.outputs;
            grounded |= outputs.grounded;
            self.glass.extend_from_slice(&outputs.glass);
            if instance
                .drawn
                .as_ref()
                .is_some_and(|drawn| drawn.layer.is_none())
            {
                self.opaque.extend_from_slice(&outputs.opaque);
            }
            self.windows.extend_from_slice(&outputs.windows);
        }
        self.hosting_order = hosting;
        damage_changed(scene, &self.glass_synced, &self.glass, |pane| pane.rect);
        damage_changed(scene, &self.windows_synced, &self.windows, |view| view.rect);
        self.glass_synced.clone_from(&self.glass);
        self.windows_synced.clone_from(&self.windows);
        if self.scale > 0.0 {
            let unscale = 1.0 / self.scale;
            for pane in self.glass.iter_mut().chain(&mut self.opaque) {
                *pane = pane.scale(unscale);
            }
            for window in &mut self.windows {
                *window = window.clone().scaled(unscale);
            }
        }
        let whole = RoundedRect::rect(Rect::from_origin_size(Point::ZERO, self.viewport));
        if !grounded && background.a > 0.0 && background.a < 1.0 {
            self.glass.push(whole);
        }
        if background.a >= 1.0 {
            self.opaque.push(whole);
        }
    }
}

fn damage_changed<T: PartialEq>(
    scene: &mut Scene,
    synced: &[T],
    current: &[T],
    area: impl Fn(&T) -> Rect,
) {
    for index in 0..synced.len().max(current.len()) {
        let (was, is) = (synced.get(index), current.get(index));
        if was != is {
            for item in was.into_iter().chain(is) {
                scene.damage_rect(area(item));
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Invalidation {
    pub(crate) root: bool,
    pub(crate) everything: bool,
}

fn settle_root(cx: &mut ReconcileCx<'_>, root: Node) {
    let last = last_of(cx.layouts, root);
    cx.table.labels.attach_after(None, root, last);
    cx.table.islands.set_root(root);
}
