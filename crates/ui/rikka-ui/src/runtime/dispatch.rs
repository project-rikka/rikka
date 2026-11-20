use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

use rikka_input::Command;
use rikka_types::{CursorShape, Key, Point, PointerButton, Size, Stylus};
use smallvec::SmallVec;

use crate::input::{CaptureRequest, DispatchShared, Event, FocusRequest, RawEventCtx};
use crate::runtime::handle::Marks;
use crate::runtime::latch::ScrollStep;
use crate::runtime::layout::{Layouts, Node};
use crate::runtime::presses::Presses;
use crate::runtime::route::HitRoute;
use crate::runtime::table::{ENTERED, PAINT, Table};
use crate::runtime::tree::UiTree;
use crate::runtime::zoom::into_subtree;
use crate::view::{Behavior, Scoped, State};

pub(crate) struct Shared<'a> {
    shared: &'a mut DispatchShared,
    root: &'a mut dyn State,
    stylus: Option<&'a Stylus>,
}

impl std::ops::Deref for Shared<'_> {
    type Target = DispatchShared;

    fn deref(&self) -> &DispatchShared {
        self.shared
    }
}

impl std::ops::DerefMut for Shared<'_> {
    fn deref_mut(&mut self) -> &mut DispatchShared {
        self.shared
    }
}

impl UiTree {
    pub(crate) fn dispatch(
        &mut self,
        event: &Event,
        stylus: Option<&Stylus>,
        root: &mut dyn State,
    ) {
        self.unclaimed = None;
        self.hush_tips(event);
        let mut shared = DispatchShared::default();
        let shared = &mut Shared {
            shared: &mut shared,
            root,
            stylus,
        };

        if let Event::PointerPressed {
            position, button, ..
        } = event
        {
            self.presses.press(*button, *position, Instant::now());
        }
        if let Event::PointerPressed { position, .. } | Event::PointerReleased { position, .. } =
            event
        {
            self.scroll_anchors.pressed(*position);
        }

        match event {
            Event::PointerMoved(position) => {
                self.cursor = Some(*position);
                if self.capture.is_some() {
                    self.deliver_to_capture(event, shared);
                } else {
                    self.sync_hover(shared);
                    self.point_tips(true);
                    self.deliver_hovered(event, shared);
                }
            }
            Event::PointerTravelled(_) => {
                if self.locked {
                    self.deliver_to_capture(event, shared);
                }
            }
            Event::PointerLeft => {
                self.cursor = None;
                self.tips.leave();
                if self.capture.is_none() {
                    self.sync_hover(shared);
                }
            }
            Event::PointerPressed { position, .. } if self.is_outside_modals(*position) => {
                let captor = self.capture;
                if captor.is_some() {
                    self.deliver_to_capture(event, shared);
                }
                if !shared.handled {
                    self.deliver_to_modals(event, captor, shared);
                }
                if !shared.handled {
                    self.deliver_at(*position, event, shared, None);
                }
            }
            Event::PointerPressed {
                position, button, ..
            } => {
                let command = Command::for_button(*button);
                self.deliver_press(*position, *button, command, event, shared);
            }
            Event::PointerReleased { position, .. } => {
                self.deliver_pointer(*position, event, shared, None);
            }
            Event::Scroll {
                position,
                source,
                modifiers,
                ..
            } => {
                let step = ScrollStep {
                    source: *source,
                    at: *position,
                    modifiers: *modifiers,
                    now: Instant::now(),
                };
                self.deliver_scroll(event, step, shared);
            }
            Event::Gesture { position, .. } => {
                self.deliver_pointer(*position, event, shared, None);
            }
            Event::KeyPressed {
                key: Key::Tab,
                modifiers,
                ..
            } if !modifiers.control && !self.focus_takes_tab() => {
                self.move_focus(!modifiers.shift, shared);
            }
            Event::KeyPressed { key, modifiers, .. } => {
                self.deliver_to_focus(event, shared, Command::for_key(*key, *modifiers));
            }
            Event::KeyReleased { .. }
            | Event::Preedit { .. }
            | Event::Commit(_)
            | Event::Pad(_) => {
                self.deliver_to_focus(event, shared, None);
            }
            Event::DragMoved { position, .. } => {
                self.deliver_at(*position, event, shared, None);
                self.settle_drag_target(shared);
            }
            Event::DragDropped { .. } => {
                let target = self.drag_target.take();
                self.drag_accept = None;
                self.deliver_to(target, event, shared);
            }
            Event::DragLeft => {
                let target = self.drag_target.take();
                self.drag_accept = None;
                self.deliver_to(target, event, shared);
            }
            Event::DragTaken | Event::DragWithin(_) => {
                let origin = self.drag_origin;
                self.deliver_to(origin, event, shared);
            }
            Event::DragEnded(_) => {
                let origin = self.drag_origin.take();
                self.deliver_to(origin, event, shared);
            }
            Event::DragReclaimed => self.broadcast(event, shared),
            Event::Command(command) => {
                let command = *command;
                self.deliver_to_focus(event, shared, None);
                if !shared.handled {
                    self.unclaimed = Some(command);
                }
            }
            Event::PointerEntered
            | Event::PointerExited
            | Event::PointerCancelled
            | Event::FocusGained
            | Event::FocusLost => {
                debug_assert!(
                    false,
                    "enter/exit, cancel and focus events are synthesized internally"
                );
            }
        }

        self.settle_requests(shared);
        self.refresh_cursor();
        self.collect(shared);
        self.settle_caret();
    }

    fn settle_requests(&mut self, shared: &mut Shared<'_>) {
        match shared.focus {
            FocusRequest::Undecided => {
                if let Some(id) = shared.pressed {
                    self.focus_pressed(id, shared);
                }
            }
            FocusRequest::Set(id) => self.focus_toward(id, shared),
            FocusRequest::Return(id) => self.return_focus(id, shared),
            FocusRequest::Keep => {}
        }
        self.settle_capture(shared);
    }

    fn settle_capture(&mut self, shared: &mut Shared<'_>) {
        match std::mem::take(&mut shared.capture) {
            CaptureRequest::Grab(id) => {
                self.capture = Some(id);
                self.exclusive = false;
                self.unlock();
            }
            CaptureRequest::Exclusive(id) => {
                self.capture = Some(id);
                self.exclusive = true;
                self.unlock();
            }
            CaptureRequest::Lock(id) => {
                self.capture = Some(id);
                self.exclusive = false;
                self.locked = true;
                self.cursor_shape = CursorShape::Hidden;
            }
            CaptureRequest::Release => {
                let interceptors_kept_out = std::mem::take(&mut self.exclusive);
                self.capture = None;
                self.locked = false;
                self.sync_hover(shared);
                self.point_tips(false);
                if interceptors_kept_out {
                    self.announce_cursor(shared);
                }
            }
            CaptureRequest::None => {}
        }
    }

    fn announce_cursor(&mut self, shared: &mut Shared<'_>) {
        let Some(cursor) = self.cursor else {
            return;
        };
        if self.hovered.is_empty() || !self.hovered_intact() {
            return;
        }
        let handled = std::mem::replace(&mut shared.handled, false);
        let stylus = shared.stylus.take();
        let route = std::mem::take(&mut self.hovered);
        let mut cx = self.dispatcher(shared);
        intercepted(route.path(), &Event::PointerMoved(cursor), &mut cx);
        self.hovered = route;
        shared.stylus = stylus;
        shared.handled = handled;
        self.settle_capture(shared);
    }

    fn return_focus(&mut self, id: u64, shared: &mut Shared<'_>) {
        let back = self
            .focus_before
            .filter(|back| *back != id && self.focusable(*back));
        if self.focus == Some(id) {
            self.set_focus(back, shared);
        } else {
            self.unpark_focus(id, back);
        }
    }

    fn refresh_cursor(&mut self) {
        let Some(point) = self.cursor else {
            return;
        };
        if self.capture.is_none() && !self.hovered.is_empty() && self.hovered_intact() {
            self.cursor_shape = cursor_along(
                &self.table,
                &self.layouts,
                &self.theme,
                point,
                self.hovered.path(),
            );
        }
    }

    pub(crate) fn refresh_hover(&mut self, root: &mut dyn State) {
        let mut shared = DispatchShared::default();
        let shared = &mut Shared {
            shared: &mut shared,
            root,
            stylus: None,
        };
        if self.capture.is_none() && self.hit_stale {
            self.hit_stale = false;
            self.sync_hover(shared);
            self.point_tips(false);
        }
        self.settle_requests(shared);
        self.collect(shared);
    }

    pub(crate) fn refresh_focus(&mut self, root: &mut dyn State) {
        let mut shared = DispatchShared::default();
        let shared = &mut Shared {
            shared: &mut shared,
            root,
            stylus: None,
        };
        if self.notified_focus != self.focus {
            let left = self.notified_focus;
            self.notified_focus = self.focus;
            self.deliver_to(left, &Event::FocusLost, shared);
            self.deliver_to(self.focus, &Event::FocusGained, shared);
        }
        self.settle_requests(shared);
        self.collect(shared);
        self.settle_caret();
    }

    fn collect(&mut self, shared: &mut Shared<'_>) {
        if shared.repaint || self.tips.unsettled() {
            self.needs_frame = true;
        }
        if shared.view {
            self.wants_view = true;
        }
        self.requests.append(&mut shared.requests);
        if let Some((origin, drag)) = shared.drag_start.take() {
            self.drag_origin = Some(origin);
            self.drag_start = Some(drag);
        }
        if let Some(window) = shared.drag_window.take() {
            self.drag_window = Some(window);
        }
    }

    fn settle_drag_target(&mut self, shared: &mut Shared<'_>) {
        let accepted = shared.drag_accept.take();
        let target = accepted.as_ref().map(|(id, _)| *id);
        if self.drag_target != target {
            let left = self.drag_target;
            self.drag_target = target;
            self.deliver_to(left, &Event::DragLeft, shared);
        }
        self.drag_accept = accepted.map(|(_, mime)| mime);
    }

    fn deliver_press(
        &mut self,
        position: Point,
        button: PointerButton,
        command: Option<Command>,
        event: &Event,
        shared: &mut Shared<'_>,
    ) {
        if Command::for_button(button).is_none() && self.capture.is_some() {
            shared.pressed = self.capture;
        }
        self.deliver_pointer(position, event, shared, command);
    }

    fn focus_toward(&mut self, id: u64, shared: &mut Shared<'_>) {
        if let Some(target) = self.focusable_toward(id)
            && self.within_top_modal(target)
        {
            self.set_focus(Some(target), shared);
        }
    }

    fn focus_pressed(&mut self, id: u64, shared: &mut Shared<'_>) {
        if self.live(id).is_none() || !self.within_top_modal(id) {
            return;
        }
        let target = self
            .focusable_toward(id)
            .filter(|target| self.within_top_modal(*target));
        self.set_focus(target, shared);
    }

    fn focusable_toward(&self, id: u64) -> Option<u64> {
        let mut at = self.live(id);
        while let Some(step) = at {
            if self.table.behavior(step).focusable() {
                return Some(self.table.get(step).id);
            }
            at = self.layouts.parent(step);
        }
        None
    }

    fn deliver_pointer(
        &mut self,
        position: Point,
        event: &Event,
        shared: &mut Shared<'_>,
        command: Option<Command>,
    ) {
        if self.capture.is_some() {
            self.deliver_to_capture(event, shared);
        } else {
            self.deliver_at(position, event, shared, command);
        }
    }

    fn deliver_scroll(&mut self, event: &Event, step: ScrollStep, shared: &mut Shared<'_>) {
        if self.capture.is_some() {
            self.deliver_to_capture(event, shared);
            return;
        }
        let latched = self
            .latch
            .target(step)
            .filter(|id| self.live(*id).is_some());
        let handler = match latched {
            Some(id) => {
                self.deliver_to(Some(id), event, shared);
                Some(id)
            }
            None => self.deliver_at(step.at, event, shared, None),
        };
        self.latch.update(step, handler);
    }

    fn dispatcher<'a>(&'a mut self, shared: &'a mut Shared<'_>) -> DispatchCx<'a> {
        DispatchCx {
            table: &mut self.table,
            layouts: &self.layouts,
            shared: &mut *shared.shared,
            root: &mut *shared.root,
            chain: Vec::new(),
            theme: &self.theme,
            marks: &self.marks,
            viewport: self.viewport,
            capture: self.capture,
            exclusive: self.exclusive,
            presses: &mut self.presses,
            focus: self.focus,
            handler: None,
            stylus: shared.stylus,
        }
    }

    fn deliver_at(
        &mut self,
        position: Point,
        event: &Event,
        shared: &mut Shared<'_>,
        command: Option<Command>,
    ) -> Option<u64> {
        let anchored = self.has_anchors();
        let root = self.root?;
        let mut handler = None;
        let mut route = std::mem::take(&mut self.probe);
        if route.hit(&self.table, &self.layouts, root, Some(position), anchored) {
            if let Event::PointerPressed { button, .. } = event
                && Command::for_button(*button).is_none()
            {
                self.pressed.clear();
                self.pressed.extend_from_slice(route.ids());
                shared.pressed = route.ids().last().copied();
            }
            let mut cx = self.dispatcher(shared);
            let claimed = match event {
                Event::DragMoved { .. } => {
                    drag_along(route.path(), event, &mut cx);
                    true
                }
                _ => dispatch_path(route.path(), event, &mut cx, Bubble::Up(command)),
            };
            handler = cx.handler;
            if let (Some(command), false) = (command, claimed) {
                self.unclaimed = Some(command);
            }
        }
        self.probe = route;
        handler
    }

    fn broadcast(&mut self, event: &Event, shared: &mut Shared<'_>) {
        let Some(root) = self.root else {
            return;
        };
        let mut cx = self.dispatcher(shared);
        broadcast_walk(root, event, &mut cx);
    }

    fn deliver_hovered(&mut self, event: &Event, shared: &mut Shared<'_>) {
        if self.hovered.is_empty() || !self.hovered_intact() {
            return;
        }
        let route = std::mem::take(&mut self.hovered);
        let mut cx = self.dispatcher(shared);
        dispatch_path(route.path(), event, &mut cx, Bubble::Up(None));
        self.hovered = route;
    }

    fn hovered_intact(&self) -> bool {
        intact(&self.table, &self.layouts, &self.hovered)
    }

    fn is_outside_modals(&self, position: Point) -> bool {
        if self.modals.is_empty() || self.capture.is_some_and(|id| !self.within_modal(id)) {
            return false;
        }
        !self.modals.iter().any(|id| {
            self.live(*id)
                .is_some_and(|node| self.table.contains(&self.layouts, node, position))
        })
    }

    fn within_modal(&self, id: u64) -> bool {
        let mut at = self.live(id);
        while let Some(node) = at {
            if self.modals.contains(&self.table.get(node).id) {
                return true;
            }
            at = self.layouts.parent(node);
        }
        false
    }

    fn deliver_to_modals(&mut self, event: &Event, captor: Option<u64>, shared: &mut Shared<'_>) {
        for index in 0..self.modals.len() {
            let id = self.modals[index];
            if Some(id) != captor {
                self.deliver_to(Some(id), event, shared);
            }
        }
    }

    fn deliver_to_capture(&mut self, event: &Event, shared: &mut Shared<'_>) {
        self.deliver_to(self.capture, event, shared);
    }

    fn deliver_to_focus(
        &mut self,
        event: &Event,
        shared: &mut Shared<'_>,
        command: Option<Command>,
    ) {
        let target = self.keyboard_target();
        let bubble = match self.top_modal_node() {
            Some(modal) => Bubble::Within(command, modal),
            None => Bubble::Up(command),
        };
        let claimed = self.dispatch_to(target, event, shared, bubble);
        if let (Some(command), false) = (command, claimed) {
            self.unclaimed = Some(command);
        }
    }

    fn keyboard_target(&self) -> Option<u64> {
        let target = self
            .focus
            .or_else(|| {
                self.pressed
                    .iter()
                    .rev()
                    .copied()
                    .find(|id| self.table.find(*id).is_some())
            })
            .or_else(|| self.root.map(|root| self.table.get(root).id));
        match self.top_modal_node() {
            Some(modal) if !target.is_some_and(|id| self.within_top_modal(id)) => {
                Some(self.table.get(modal).id)
            }
            _ => target,
        }
    }

    fn deliver_to(&mut self, target: Option<u64>, event: &Event, shared: &mut Shared<'_>) {
        self.dispatch_to(target, event, shared, Bubble::Not);
    }

    fn dispatch_to(
        &mut self,
        target: Option<u64>,
        event: &Event,
        shared: &mut Shared<'_>,
        bubble: Bubble,
    ) -> bool {
        let Some(id) = target else {
            return false;
        };
        let Some(path) = self.path_to(id) else {
            return false;
        };
        let mut cx = self.dispatcher(shared);
        dispatch_path(&path, event, &mut cx, bubble)
    }

    fn move_focus(&mut self, forward: bool, shared: &mut Shared<'_>) {
        let mut order = Vec::new();
        if let Some(scope) = self.top_modal_node().or(self.root) {
            collect_tab_stops(&self.table, &self.layouts, scope, &mut order);
        }
        if order.is_empty() {
            self.set_focus(None, shared);
            return;
        }
        let next = match self
            .focus
            .and_then(|id| order.iter().position(|&candidate| candidate == id))
        {
            Some(index) if forward => (index + 1) % order.len(),
            Some(index) => (index + order.len() - 1) % order.len(),
            None if forward => 0,
            None => order.len() - 1,
        };
        self.set_focus(Some(order[next]), shared);
        self.focus_tips();
    }

    fn set_focus(&mut self, focus: Option<u64>, shared: &mut Shared<'_>) {
        if self.focus != focus {
            self.focus_before = self.focus;
            self.set_focused(focus);
            shared.repaint = true;
        }
    }

    fn sync_hover(&mut self, shared: &mut Shared<'_>) {
        let cursor = self.cursor;
        self.cursor_shape = CursorShape::Default;
        let anchored = self.has_anchors();
        let Some(root) = self.root else {
            self.hovered = HitRoute::default();
            return;
        };
        let mut next = std::mem::take(&mut self.probe);
        let hit = next.hit(&self.table, &self.layouts, root, cursor, anchored);
        let keep = self.hovered.shared_depth(&next);
        let intact = self.hovered_intact();
        let hovered = std::mem::take(&mut self.hovered);
        let mut cx = self.dispatcher(shared);
        if intact {
            exit_along(&hovered, keep, &mut cx);
            if hit {
                enter_along(next.path(), keep, &mut cx);
            }
        } else {
            hover_anew(if hit { next.path() } else { &[] }, &mut cx);
        }
        if let (Some(point), true) = (cursor, hit) {
            self.cursor_shape =
                cursor_along(&self.table, &self.layouts, &self.theme, point, next.path());
        }
        self.hovered = next;
        self.probe = hovered;
    }
}

fn intact(table: &Table, layouts: &Layouts, route: &HitRoute) -> bool {
    let path = route.path();
    path.iter()
        .zip(route.ids())
        .all(|(node, id)| table.find(*id) == Some(*node))
        && path
            .windows(2)
            .all(|pair| layouts.parent(pair[1]) == Some(pair[0]))
}

fn cursor_along(
    table: &Table,
    layouts: &Layouts,
    theme: &crate::ambient::ThemeHandle,
    point: Point,
    path: &[Node],
) -> CursorShape {
    let Some(&first) = path.first() else {
        return CursorShape::default();
    };
    let mut seen = table.seen(layouts, first);
    let mut shape = None;
    for &node in path {
        let instance = table.get(node);
        shape = instance
            .widget
            .cursor(
                &*instance.state,
                seen.bounds(table, node),
                point,
                theme,
                &instance.env,
            )
            .or(shape);
        seen = seen.within(table, node);
    }
    shape.unwrap_or_default()
}

struct DispatchCx<'a> {
    table: &'a mut Table,
    layouts: &'a Layouts,
    shared: &'a mut DispatchShared,
    root: &'a mut dyn State,
    chain: Vec<Rc<Scoped>>,
    theme: &'a crate::ambient::ThemeHandle,
    marks: &'a Arc<Marks>,
    viewport: Size,
    capture: Option<u64>,
    exclusive: bool,
    presses: &'a mut Presses,
    focus: Option<u64>,
    handler: Option<u64>,
    stylus: Option<&'a Stylus>,
}

impl DispatchCx<'_> {
    fn enter(&mut self, node: Node) -> usize {
        let depth = self.chain.len();
        self.chain
            .extend(self.table.get(node).scopes.iter().cloned());
        depth
    }

    fn holds_exclusively(&self, path: &[Node]) -> bool {
        self.exclusive
            && path
                .last()
                .is_some_and(|node| Some(self.table.get(*node).id) == self.capture)
    }

    fn enter_path(&mut self, path: &[Node]) {
        self.chain.clear();
        for node in path {
            self.enter(*node);
        }
    }
}

fn collect_tab_stops(table: &Table, layouts: &Layouts, node: Node, out: &mut Vec<u64>) {
    let behavior = table.behavior(node);
    if behavior.has(Behavior::INERT) {
        return;
    }
    if behavior.tab_stop() {
        out.push(table.get(node).id);
    }
    for &child in layouts.children(node) {
        collect_tab_stops(table, layouts, child, out);
    }
}

fn exit_along(route: &HitRoute, keep: usize, cx: &mut DispatchCx<'_>) {
    let path = route.path();
    for depth in (keep..path.len()).rev() {
        let node = path[depth];
        if !cx.table.leave(node) {
            continue;
        }
        cx.enter_path(&path[..=depth]);
        notify_hover(node, &Event::PointerExited, cx);
    }
    cx.chain.clear();
}

fn enter_along(path: &[Node], from: usize, cx: &mut DispatchCx<'_>) {
    for depth in (from..path.len()).rev() {
        let node = path[depth];
        if !cx.table.enter(node) {
            continue;
        }
        cx.enter_path(&path[..=depth]);
        notify_hover(node, &Event::PointerEntered, cx);
    }
    cx.chain.clear();
}

fn hover_anew(path: &[Node], cx: &mut DispatchCx<'_>) {
    let mut left: Vec<Node> = cx
        .table
        .entered()
        .iter()
        .copied()
        .filter(|node| !path.contains(node))
        .collect();
    left.sort_by_key(|node| std::cmp::Reverse(cx.table.labels.label(*node)));
    let mut above = Vec::new();
    for node in left {
        cx.table.leave(node);
        above.clear();
        cx.layouts.lineage(node, &mut above);
        cx.enter_path(&above);
        notify_hover(node, &Event::PointerExited, cx);
    }
    enter_along(path, 0, cx);
}

fn notify_hover(node: Node, event: &Event, cx: &mut DispatchCx<'_>) {
    if cx.table.behavior(node).has(Behavior::TRACKS_HOVER) {
        cx.shared.repaint = true;
    }
    deliver(node, event, cx);
    cx.shared.handled = false;
}

#[derive(Clone, Copy)]
enum Bubble {
    Not,
    Up(Option<Command>),
    Within(Option<Command>, Node),
}

fn dispatch_path(path: &[Node], event: &Event, cx: &mut DispatchCx<'_>, bubble: Bubble) -> bool {
    let streams = is_pointer_stream(event);
    if streams && !cx.holds_exclusively(path) && intercepted(path, event, cx) {
        return true;
    }
    cx.chain.clear();
    let mut depths: SmallVec<[usize; 32]> = SmallVec::with_capacity(path.len());
    for node in path {
        depths.push(cx.enter(*node));
    }
    let Some((&target, above)) = path.split_last() else {
        return false;
    };
    let (command, ceiling) = match bubble {
        Bubble::Not => (None, Some(target)),
        Bubble::Up(command) => (command, None),
        Bubble::Within(command, ceiling) => (command, Some(ceiling)),
    };
    deliver_meaning(target, event, command, cx);
    if ceiling != Some(target) {
        for (at, node) in above.iter().enumerate().rev() {
            if cx.shared.handled {
                break;
            }
            let watched = streams && cx.table.behavior(*node).has(Behavior::INTERCEPTS);
            if !watched {
                let reach = depths.get(at + 1).copied().unwrap_or(cx.chain.len());
                cx.chain.truncate(reach);
                deliver_meaning(*node, event, command, cx);
            }
            if ceiling == Some(*node) {
                break;
            }
        }
    }
    cx.chain.clear();
    cx.shared.handled
}

fn is_pointer_stream(event: &Event) -> bool {
    matches!(
        event,
        Event::PointerPressed { .. } | Event::PointerMoved(_) | Event::PointerReleased { .. }
    )
}

fn carries_stylus(event: &Event) -> bool {
    is_pointer_stream(event) || matches!(event, Event::Scroll { .. })
}

fn intercepted(path: &[Node], event: &Event, cx: &mut DispatchCx<'_>) -> bool {
    let Some((_, above)) = path.split_last() else {
        return false;
    };
    let floor = path
        .iter()
        .rposition(|node| cx.table.behavior(*node).has(Behavior::MODAL))
        .unwrap_or(0)
        .min(above.len());
    let intercepts = |node: &Node| cx.table.behavior(*node).has(Behavior::INTERCEPTS);
    if !above[floor..].iter().any(intercepts) {
        return false;
    }
    cx.chain.clear();
    for (at, &node) in above.iter().enumerate() {
        cx.enter(node);
        if at < floor || !cx.table.behavior(node).has(Behavior::INTERCEPTS) {
            continue;
        }
        let id = cx.table.get(node).id;
        let handled = cx.shared.handled;
        let handler = cx.handler;
        deliver(node, event, cx);
        cx.shared.handled = handled;
        cx.handler = handler;
        let claimed = matches!(
            cx.shared.capture,
            CaptureRequest::Grab(claimer) | CaptureRequest::Exclusive(claimer) if claimer == id
        );
        if claimed {
            let claim = std::mem::take(&mut cx.shared.capture);
            cancel_beneath(path, at, cx);
            cx.shared.capture = claim;
            cx.shared.handled = true;
            cx.handler = Some(id);
            cx.chain.clear();
            return true;
        }
    }
    cx.chain.clear();
    false
}

fn cancel_beneath(path: &[Node], claimer: usize, cx: &mut DispatchCx<'_>) {
    cx.chain.clear();
    let mut depths: SmallVec<[usize; 32]> = SmallVec::with_capacity(path.len());
    for node in path {
        depths.push(cx.enter(*node));
    }
    for at in (claimer + 1..path.len()).rev() {
        let reach = depths.get(at + 1).copied().unwrap_or(cx.chain.len());
        cx.chain.truncate(reach);
        deliver(path[at], &Event::PointerCancelled, cx);
    }
}

fn deliver_meaning(node: Node, event: &Event, command: Option<Command>, cx: &mut DispatchCx<'_>) {
    deliver(node, event, cx);
    if let (Some(command), false) = (command, cx.shared.handled) {
        deliver(node, &Event::Command(command), cx);
    }
}

fn drag_along(path: &[Node], event: &Event, cx: &mut DispatchCx<'_>) {
    cx.chain.clear();
    drag_step(path, event, cx);
    cx.chain.clear();
}

fn drag_step(path: &[Node], event: &Event, cx: &mut DispatchCx<'_>) {
    let Some((&node, rest)) = path.split_first() else {
        return;
    };
    let scopes = cx.enter(node);
    let catches = cx.table.behavior(node).has(Behavior::CATCHES_DROPS);
    if catches {
        deliver(node, event, cx);
    }
    if !catches || cx.shared.drag_accept.is_none() {
        match rest.is_empty() {
            false => {
                drag_step(rest, event, cx);
                if !catches && !cx.shared.handled {
                    deliver(node, event, cx);
                }
            }
            true if !catches => deliver(node, event, cx),
            true => {}
        }
    }
    cx.chain.truncate(scopes);
}

fn broadcast_walk(node: Node, event: &Event, cx: &mut DispatchCx<'_>) -> bool {
    let scopes = cx.enter(node);
    deliver(node, event, cx);
    let mut handled = cx.shared.handled;
    let layouts = cx.layouts;
    for &child in layouts.children(node) {
        if handled {
            break;
        }
        handled = broadcast_walk(child, event, cx);
    }
    cx.chain.truncate(scopes);
    handled
}

fn deliver(node: Node, event: &Event, cx: &mut DispatchCx<'_>) {
    let stylus = cx.stylus.filter(|_| carries_stylus(event));
    let delivered = into_subtree(cx.table, cx.layouts, node, event, stylus);
    let event = delivered.event.as_ref();
    let bounds = cx.table.bounds(cx.layouts, node);
    let hovered = cx.table.has(node, ENTERED);
    let unhandled = !cx.shared.handled;
    let instance = cx.table.get_mut(node);
    let room = instance.room(bounds);
    if instance.drawn.is_some() {
        instance.relocate(bounds.origin);
    }
    let id = instance.id;
    let presses = match event {
        Event::PointerPressed { .. } => cx.presses.receive(id),
        _ => 0,
    };
    let raw = RawEventCtx {
        instance: id,
        bounds,
        room,
        content_size: instance.content_size,
        viewport: cx.viewport,
        theme: cx.theme,
        env: instance.env,
        hovered,
        captured: cx.capture == Some(id),
        presses,
        focused: cx.focus == Some(id),
        shared: cx.shared,
        root: cx.root,
        chain: &cx.chain,
        marks: cx.marks,
        stylus: delivered.stylus.as_deref(),
    };
    instance.widget.event(instance.state.as_mut(), event, raw);
    if unhandled && cx.shared.handled {
        cx.handler = Some(id);
    }
    cx.table.refresh_behavior(node);
    cx.table.mark(cx.layouts, node, PAINT);
}
