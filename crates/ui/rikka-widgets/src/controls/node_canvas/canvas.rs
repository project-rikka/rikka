use std::sync::Arc;

use rikka_types::{CursorShape, Key, Modifiers, Point, PointerButton, Rect, ScrollDelta, Size};
use rikka_ui::{
    Action, Arranged, Behavior, Command, Element, Event, EventCtx, Gesture, IntoHandler, Layout,
    Length, PaintCtx, Quad, ScrollSource, Style, Styled, Tooltip, Widget, palette,
};
use rustc_hash::FxHashMap;

use crate::controls::node_canvas::card::{Card, Lit, badge_area};
use crate::controls::node_canvas::geometry::{CanvasView, Geometry, Hit, Toward};
use crate::controls::node_canvas::layers::{Frame, Overlay, Sketch, Wires};
use crate::controls::node_canvas::model::{
    CanvasLink, FrameArea, FrameKey, NodeGraph, NodeKey, PortAddr, PortSide,
};
use crate::controls::node_canvas::selection::{Extend, Item, Selection};

const GRID: f32 = 24.0;
const MAJOR_EVERY: i32 = 5;
const SPARSEST_GRID: f32 = 9.0;
const ZOOM_STEP: f32 = 1.2;
const NUDGE: f32 = 8.0;
const DRAG_SLOP: f32 = 3.0;
const KEY_PAN: f32 = 48.0;

#[derive(Clone, Debug, PartialEq)]
pub enum CanvasEdit {
    Move(Vec<(NodeKey, Point)>),
    MoveFrame(FrameKey, Point),
    ResizeFrame(FrameKey, Size),
    Connect {
        from: PortAddr,
        to: PortAddr,
        replacing: Option<PortAddr>,
    },
    Disconnect(PortAddr),
    Delete(Selection),
    Copy(Selection),
    Cut(Selection),
    Duplicate(Selection),
    Paste(Point),
    Undo,
    Redo,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanvasMenu {
    pub at: Point,
    pub local: Point,
    pub hit: Hit,
    pub from: Option<PortAddr>,
}

#[must_use]
#[derive(Widget)]
pub struct NodeCanvas {
    graph: Arc<NodeGraph>,
    #[debug]
    selection: Selection,
    on_select: Action<(Selection,)>,
    on_edit: Action<(CanvasEdit,)>,
    on_menu: Action<(CanvasMenu,)>,
    on_activate: Action<(Item,)>,
    #[style]
    style: Style,
}

pub fn node_canvas(graph: Arc<NodeGraph>, selection: Selection) -> NodeCanvas {
    NodeCanvas {
        graph,
        selection,
        on_select: Action::default(),
        on_edit: Action::default(),
        on_menu: Action::default(),
        on_activate: Action::default(),
        style: Style::default(),
    }
}

impl NodeCanvas {
    pub fn on_select<Marker>(
        mut self,
        handler: impl IntoHandler<(Selection,), (), Marker>,
    ) -> Self {
        self.on_select = Action::new(handler);
        self
    }

    pub fn on_edit<Marker>(mut self, handler: impl IntoHandler<(CanvasEdit,), (), Marker>) -> Self {
        self.on_edit = Action::new(handler);
        self
    }

    pub fn on_menu<Marker>(mut self, handler: impl IntoHandler<(CanvasMenu,), (), Marker>) -> Self {
        self.on_menu = Action::new(handler);
        self
    }

    pub fn on_activate<Marker>(mut self, handler: impl IntoHandler<(Item,), (), Marker>) -> Self {
        self.on_activate = Action::new(handler);
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Drag {
    Pan {
        from: Point,
        pan: Point,
    },
    Marquee {
        from: Point,
        to: Point,
        extend: Extend,
    },
    Nodes {
        from: Point,
        by: Point,
        keys: Vec<NodeKey>,
        settle: Option<Item>,
    },
    Link {
        fixed: PortAddr,
        pointer: Point,
        detached: Option<PortAddr>,
        target: Option<PortAddr>,
    },
    Frame {
        key: FrameKey,
        from: Point,
        by: Point,
        members: Vec<NodeKey>,
    },
    Resize {
        key: FrameKey,
        from: Point,
        by: Point,
    },
}

struct Laid {
    graph: Arc<NodeGraph>,
    moving: Vec<NodeKey>,
    by: Point,
    geometry: Arc<Geometry>,
}

struct Wiring {
    inputs: Vec<bool>,
    outputs: Vec<bool>,
}

#[doc(hidden)]
#[derive(Default)]
pub struct NodeCanvasState {
    view: CanvasView,
    framed: bool,
    drag: Option<Drag>,
    hover: Option<Hit>,
    focus: Option<NodeKey>,
    pointer: Option<Point>,
    geometry: Option<Laid>,
    room: Size,
}

impl std::fmt::Debug for NodeCanvasState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NodeCanvasState")
            .field("view", &self.view)
            .field("drag", &self.drag)
            .field("focus", &self.focus)
            .finish_non_exhaustive()
    }
}

impl NodeCanvasState {
    fn moving(&self) -> (&[NodeKey], Point) {
        match &self.drag {
            Some(Drag::Nodes { keys, by, .. }) => (keys, *by),
            Some(Drag::Frame { members, by, .. }) => (members, *by),
            _ => (&[], Point::ZERO),
        }
    }

    fn geometry(&mut self, graph: &Arc<NodeGraph>) -> Arc<Geometry> {
        let (moving, by) = self.moving();
        if let Some(laid) = &self.geometry
            && Arc::ptr_eq(&laid.graph, graph)
            && laid.moving == moving
            && laid.by == by
        {
            return Arc::clone(&laid.geometry);
        }
        let geometry = Arc::new(Geometry::moved(graph, moving, by));
        let moving = moving.to_vec();
        self.geometry = Some(Laid {
            graph: Arc::clone(graph),
            moving,
            by,
            geometry: Arc::clone(&geometry),
        });
        geometry
    }
}

fn wiring(graph: &NodeGraph) -> Vec<Wiring> {
    let mut wiring: Vec<Wiring> = graph
        .nodes
        .iter()
        .map(|node| Wiring {
            inputs: vec![false; node.inputs.len()],
            outputs: vec![false; node.outputs.len()],
        })
        .collect();
    let index: FxHashMap<NodeKey, usize> = graph
        .nodes
        .iter()
        .enumerate()
        .map(|(at, node)| (node.key, at))
        .collect();
    for link in &graph.links {
        for (end, side) in [(link.to, PortSide::Input), (link.from, PortSide::Output)] {
            let Some(&at) = index.get(&end.node) else {
                continue;
            };
            let ports = match side {
                PortSide::Input => &mut wiring[at].inputs,
                PortSide::Output => &mut wiring[at].outputs,
            };
            if let Some(slot) = ports.get_mut(end.index) {
                *slot = true;
            }
        }
    }
    wiring
}

fn picked_up(graph: &NodeGraph, input: PortAddr) -> Option<PortAddr> {
    let many = graph
        .nodes
        .iter()
        .find(|node| node.key == input.node)
        .and_then(|node| node.inputs.get(input.index))
        .is_some_and(|port| port.many);
    if many {
        return None;
    }
    graph
        .links
        .iter()
        .find(|link| link.to == input)
        .map(|link| link.from)
}

impl NodeCanvas {
    fn local(ctx_bounds: Rect, position: Point) -> Point {
        position - ctx_bounds.origin
    }

    fn hit(&self, state: &mut NodeCanvasState, local: Point) -> Hit {
        let geometry = state.geometry(&self.graph);
        geometry.hit(&self.graph, &state.view, local)
    }

    fn select(&self, selection: Selection, ctx: &mut EventCtx<'_>) {
        if selection != self.selection {
            ctx.act(&self.on_select, (selection,));
        }
    }

    fn edit(&self, edit: CanvasEdit, ctx: &mut EventCtx<'_>) {
        ctx.act(&self.on_edit, (edit,));
    }

    fn set_view(&self, state: &mut NodeCanvasState, view: CanvasView, ctx: &mut EventCtx<'_>) {
        if view == state.view {
            return;
        }
        if view.zoom != state.view.zoom {
            ctx.handle().rebuild();
        }
        state.view = view;
        ctx.request_paint();
    }

    fn frame_all(&self, state: &mut NodeCanvasState) {
        let geometry = state.geometry(&self.graph);
        state.view = match geometry.bounds() {
            Some(area) => CanvasView::framing(area, state.room),
            None => CanvasView {
                pan: Point::new(state.room.width * 0.5, state.room.height * 0.5),
                zoom: 1.0,
            },
        };
    }

    fn link_target(
        &self,
        state: &mut NodeCanvasState,
        fixed: PortAddr,
        local: Point,
    ) -> Option<PortAddr> {
        match self.hit(state, local) {
            Hit::Port(port) if port.node != fixed.node && port.side != fixed.side => Some(port),
            _ => None,
        }
    }

    fn press(
        &self,
        state: &mut NodeCanvasState,
        local: Point,
        button: PointerButton,
        modifiers: Modifiers,
        ctx: &mut EventCtx<'_>,
    ) {
        let at = state.view.to_graph(local);
        let hit = self.hit(state, local);
        match button {
            PointerButton::Middle => {
                state.drag = Some(Drag::Pan {
                    from: local,
                    pan: state.view.pan,
                });
            }
            PointerButton::Secondary => {
                ctx.act(
                    &self.on_menu,
                    (CanvasMenu {
                        at,
                        local,
                        hit,
                        from: None,
                    },),
                );
                return;
            }
            PointerButton::Primary => {
                let extend = Extend::from_modifiers(modifiers);
                if ctx.presses() >= 2 {
                    let item = match hit {
                        Hit::Node(key) => Some(Item::Node(key)),
                        Hit::FrameTitle(key) | Hit::Frame(key) => Some(Item::Frame(key)),
                        Hit::Link(link) => Some(Item::Link(link)),
                        _ => None,
                    };
                    if let Some(item) = item {
                        ctx.act(&self.on_activate, (item,));
                        return;
                    }
                }
                state.drag = Some(match hit {
                    Hit::Port(port) => {
                        let source = (port.side == PortSide::Input)
                            .then(|| picked_up(&self.graph, port))
                            .flatten();
                        match source {
                            Some(from) => Drag::Link {
                                fixed: from,
                                pointer: at,
                                detached: Some(port),
                                target: Some(port),
                            },
                            None => Drag::Link {
                                fixed: port,
                                pointer: at,
                                detached: None,
                                target: None,
                            },
                        }
                    }
                    Hit::Node(key) => {
                        let item = Item::Node(key);
                        let selection = self.selection.pressed(item, extend);
                        let keys = selection.nodes().collect();
                        let settle = (selection == self.selection && extend == Extend::Replace)
                            .then_some(item);
                        state.focus = Some(key);
                        self.select(selection, ctx);
                        Drag::Nodes {
                            from: at,
                            by: Point::ZERO,
                            keys,
                            settle,
                        }
                    }
                    Hit::Link(link) => {
                        self.select(self.selection.picked(Item::Link(link), extend), ctx);
                        return;
                    }
                    Hit::FrameTitle(key) => {
                        self.select(self.selection.pressed(Item::Frame(key), extend), ctx);
                        let geometry = state.geometry(&self.graph);
                        Drag::Frame {
                            key,
                            from: at,
                            by: Point::ZERO,
                            members: geometry.frame_members(&self.graph, key),
                        }
                    }
                    Hit::FrameGrip(key) => Drag::Resize {
                        key,
                        from: at,
                        by: Point::ZERO,
                    },
                    Hit::Frame(_) | Hit::Empty if modifiers.alt => Drag::Pan {
                        from: local,
                        pan: state.view.pan,
                    },
                    Hit::Frame(_) | Hit::Empty => Drag::Marquee {
                        from: at,
                        to: at,
                        extend,
                    },
                });
            }
            _ => return,
        }
        ctx.capture_pointer();
        ctx.handle().rebuild();
        ctx.request_paint();
        ctx.set_handled();
    }

    fn drag_to(&self, state: &mut NodeCanvasState, local: Point, ctx: &mut EventCtx<'_>) {
        let at = state.view.to_graph(local);
        let view = state.view;
        let mut target_of = None;
        if let Some(Drag::Link { fixed, .. }) = &state.drag {
            target_of = Some(self.link_target(state, *fixed, local));
        }
        let Some(drag) = state.drag.as_mut() else {
            return;
        };
        match drag {
            Drag::Pan { from, pan } => {
                let pan = *pan + (local - *from);
                state.view = CanvasView {
                    pan,
                    zoom: view.zoom,
                };
                ctx.request_paint();
                return;
            }
            Drag::Marquee { to, .. } => *to = at,
            Drag::Nodes { from, by, .. }
            | Drag::Frame { from, by, .. }
            | Drag::Resize { from, by, .. } => {
                let moved = at - *from;
                let slop = DRAG_SLOP / view.zoom;
                if *by == Point::ZERO && moved.x.abs() < slop && moved.y.abs() < slop {
                    return;
                }
                *by = moved;
            }
            Drag::Link {
                pointer, target, ..
            } => {
                *pointer = at;
                *target = target_of.flatten();
            }
        }
        ctx.handle().rebuild();
    }

    fn release(&self, state: &mut NodeCanvasState, ctx: &mut EventCtx<'_>) {
        let Some(drag) = state.drag.take() else {
            return;
        };
        ctx.release_pointer();
        ctx.keep_focus();
        ctx.handle().rebuild();
        ctx.request_paint();
        match drag {
            Drag::Pan { .. } => {}
            Drag::Marquee { from, to, extend } => {
                let area = Rect::from_origin_size(
                    from.min(to),
                    Size::new((to.x - from.x).abs(), (to.y - from.y).abs()),
                );
                let geometry = state.geometry(&self.graph);
                let inside: Vec<Item> = geometry.nodes_within(area).map(Item::Node).collect();
                self.select(self.selection.boxed(inside, extend), ctx);
            }
            Drag::Nodes {
                by, keys, settle, ..
            } => {
                if by == Point::ZERO {
                    if let Some(item) = settle {
                        self.select(self.selection.picked(item, Extend::Replace), ctx);
                    }
                    return;
                }
                self.move_nodes(&keys, by, ctx);
            }
            Drag::Frame {
                key, by, members, ..
            } => {
                if by == Point::ZERO {
                    return;
                }
                match self.frame_area(key) {
                    Some(FrameArea::At { position, .. }) => {
                        self.edit(CanvasEdit::MoveFrame(key, position + by), ctx)
                    }
                    _ => self.move_nodes(&members, by, ctx),
                }
            }
            Drag::Resize { key, by, .. } => {
                if let Some(FrameArea::At { size, .. }) = self.frame_area(key)
                    && by != Point::ZERO
                {
                    let size = Size::new(
                        (size.width + by.x).max(crate::controls::node_canvas::geometry::GRIP * 3.0),
                        (size.height + by.y)
                            .max(crate::controls::node_canvas::geometry::FRAME_TITLE * 2.0),
                    );
                    self.edit(CanvasEdit::ResizeFrame(key, size), ctx);
                }
            }
            Drag::Link {
                fixed,
                pointer,
                detached,
                target,
            } => match (target, detached) {
                (Some(target), Some(detached)) if target == detached => {}
                (Some(target), _) => {
                    let (from, to) = match fixed.side {
                        PortSide::Output => (fixed, target),
                        PortSide::Input => (target, fixed),
                    };
                    self.edit(
                        CanvasEdit::Connect {
                            from,
                            to,
                            replacing: detached,
                        },
                        ctx,
                    );
                }
                (None, Some(detached)) => self.edit(CanvasEdit::Disconnect(detached), ctx),
                (None, None) => {
                    let local = state.view.to_local(pointer);
                    ctx.act(
                        &self.on_menu,
                        (CanvasMenu {
                            at: pointer,
                            local,
                            hit: Hit::Empty,
                            from: Some(fixed),
                        },),
                    );
                }
            },
        }
    }

    fn frame_area(&self, key: FrameKey) -> Option<FrameArea> {
        self.graph
            .frames
            .iter()
            .find(|frame| frame.key == key)
            .map(|frame| frame.area.clone())
    }

    fn move_nodes(&self, keys: &[NodeKey], by: Point, ctx: &mut EventCtx<'_>) {
        let moves: Vec<(NodeKey, Point)> = self
            .graph
            .nodes
            .iter()
            .filter(|node| keys.contains(&node.key))
            .map(|node| (node.key, node.position + by))
            .collect();
        if !moves.is_empty() {
            self.edit(CanvasEdit::Move(moves), ctx);
        }
    }

    fn hover(&self, state: &mut NodeCanvasState, local: Point, ctx: &mut EventCtx<'_>) {
        state.pointer = Some(local);
        let hit = self.hit(state, local);
        let hover = (hit != Hit::Empty).then_some(hit);
        if state.hover != hover {
            state.hover = hover;
            ctx.handle().rebuild();
        }
    }

    fn all(&self) -> Selection {
        Selection::of(
            self.graph
                .nodes
                .iter()
                .map(|node| Item::Node(node.key))
                .chain(self.graph.frames.iter().map(|frame| Item::Frame(frame.key))),
        )
    }

    fn focused_selection(&self, state: &NodeCanvasState) -> Selection {
        match (self.selection.is_empty(), state.focus) {
            (true, Some(focus)) => Selection::of([Item::Node(focus)]),
            _ => self.selection.clone(),
        }
    }

    fn paste_point(&self, state: &NodeCanvasState) -> Point {
        let local = state
            .pointer
            .unwrap_or(Point::new(state.room.width * 0.5, state.room.height * 0.5));
        state.view.to_graph(local)
    }

    fn command(
        &self,
        state: &mut NodeCanvasState,
        command: Command,
        ctx: &mut EventCtx<'_>,
    ) -> bool {
        let selection = self.focused_selection(state);
        match command {
            Command::SelectAll => self.select(self.all(), ctx),
            Command::SelectNone => self.select(Selection::default(), ctx),
            Command::Delete if !selection.is_empty() => {
                self.edit(CanvasEdit::Delete(selection), ctx)
            }
            Command::Copy if !selection.is_empty() => self.edit(CanvasEdit::Copy(selection), ctx),
            Command::Cut if !selection.is_empty() => self.edit(CanvasEdit::Cut(selection), ctx),
            Command::Paste => {
                let at = self.paste_point(state);
                self.edit(CanvasEdit::Paste(at), ctx);
            }
            Command::ZoomIn | Command::ZoomOut | Command::ZoomReset => {
                let middle = Point::new(state.room.width * 0.5, state.room.height * 0.5);
                let view = match command {
                    Command::ZoomIn => state.view.zoomed_at(middle, ZOOM_STEP),
                    Command::ZoomOut => state.view.zoomed_at(middle, 1.0 / ZOOM_STEP),
                    _ => state.view.zoomed_at(middle, 1.0 / state.view.zoom),
                };
                self.set_view(state, view, ctx);
            }
            Command::ContextMenu => {
                let at = self.paste_point(state);
                let local = state.view.to_local(at);
                let hit = state.focus.map_or(Hit::Empty, Hit::Node);
                ctx.act(
                    &self.on_menu,
                    (CanvasMenu {
                        at,
                        local,
                        hit,
                        from: None,
                    },),
                );
            }
            Command::Undo => self.edit(CanvasEdit::Undo, ctx),
            Command::Redo => self.edit(CanvasEdit::Redo, ctx),
            _ => return false,
        }
        true
    }

    fn key(
        &self,
        state: &mut NodeCanvasState,
        key: &Key,
        modifiers: Modifiers,
        ctx: &mut EventCtx<'_>,
    ) -> bool {
        let toward = match key {
            Key::ArrowLeft => Some(Toward::Left),
            Key::ArrowRight => Some(Toward::Right),
            Key::ArrowUp => Some(Toward::Up),
            Key::ArrowDown => Some(Toward::Down),
            _ => None,
        };
        if let Some(toward) = toward {
            let step = match toward {
                Toward::Left => Point::new(-1.0, 0.0),
                Toward::Right => Point::new(1.0, 0.0),
                Toward::Up => Point::new(0.0, -1.0),
                Toward::Down => Point::new(0.0, 1.0),
            };
            if modifiers.control {
                let keys: Vec<NodeKey> = self.focused_selection(state).nodes().collect();
                self.move_nodes(&keys, step * NUDGE, ctx);
            } else if modifiers.alt {
                let view = state.view.panned(step * -KEY_PAN);
                self.set_view(state, view, ctx);
            } else {
                let geometry = state.geometry(&self.graph);
                let next = match state
                    .focus
                    .filter(|focus| geometry.node_rect(*focus).is_some())
                {
                    Some(focus) => geometry.neighbour(focus, toward),
                    None => geometry.first(),
                };
                if let Some(next) = next {
                    state.focus = Some(next);
                    if modifiers.shift {
                        self.select(self.selection.picked(Item::Node(next), Extend::Add), ctx);
                    }
                    self.reveal(state, next, ctx);
                    ctx.handle().rebuild();
                }
            }
            return true;
        }
        match key {
            Key::Enter | Key::Space => {
                let Some(focus) = state.focus else {
                    return false;
                };
                let extend = match modifiers.control || modifiers.shift {
                    true => Extend::Toggle,
                    false => Extend::Replace,
                };
                self.select(self.selection.picked(Item::Node(focus), extend), ctx);
            }
            Key::Escape => {
                if state.drag.take().is_some() {
                    ctx.release_pointer();
                    ctx.handle().rebuild();
                    ctx.request_paint();
                } else if !self.selection.is_empty() {
                    self.select(Selection::default(), ctx);
                } else {
                    return false;
                }
            }
            Key::Delete | Key::Backspace => {
                return self.command(state, Command::Delete, ctx);
            }
            Key::Char('a') if modifiers.shift && !modifiers.control => {
                return self.command(state, Command::ContextMenu, ctx);
            }
            Key::Char('d') if modifiers.control || modifiers.shift => {
                let selection = self.focused_selection(state);
                if selection.is_empty() {
                    return false;
                }
                self.edit(CanvasEdit::Duplicate(selection), ctx);
            }
            Key::Char('f') | Key::Home if !modifiers.control => {
                self.frame_all(state);
                ctx.handle().rebuild();
                ctx.request_paint();
            }
            Key::Char('=') | Key::Char('+') => return self.command(state, Command::ZoomIn, ctx),
            Key::Char('-') => return self.command(state, Command::ZoomOut, ctx),
            _ => return false,
        }
        true
    }

    fn reveal(&self, state: &mut NodeCanvasState, key: NodeKey, ctx: &mut EventCtx<'_>) {
        let geometry = state.geometry(&self.graph);
        let Some(rect) = geometry.node_rect(key) else {
            return;
        };
        let shown = Rect::from_origin_size(
            state.view.to_local(rect.origin),
            Size::new(
                rect.width() * state.view.zoom,
                rect.height() * state.view.zoom,
            ),
        );
        let room = Rect::from_origin_size(Point::ZERO, state.room);
        if room.contains_rect(&shown) {
            return;
        }
        let by = Point::new(
            (room.center().x - shown.center().x).round(),
            (room.center().y - shown.center().y).round(),
        );
        let view = state.view.panned(by);
        self.set_view(state, view, ctx);
    }

    fn scroll(
        &self,
        state: &mut NodeCanvasState,
        local: Point,
        delta: ScrollDelta,
        modifiers: Modifiers,
        source: ScrollSource,
        ctx: &mut EventCtx<'_>,
    ) {
        let view = match (source, modifiers.control) {
            (ScrollSource::Wheel, _) | (_, true) => {
                state.view.zoomed_at(local, ZOOM_STEP.powf(delta.zoom()))
            }
            (ScrollSource::Continuous(_), false) => state.view.panned(delta.content()),
        };
        self.set_view(state, view, ctx);
    }
}

impl Widget for NodeCanvas {
    type State = NodeCanvasState;

    const ARRANGES: bool = true;

    fn layout(&self) -> Layout {
        Layout::column()
            .size(Length::FILL, Length::FILL)
            .with(&self.style)
    }

    fn behavior(&self, _state: &NodeCanvasState) -> Behavior {
        Behavior::CLIPS | Behavior::TRACKS_HOVER | Behavior::FOCUSABLE
    }

    fn children_offset(&self, state: &NodeCanvasState, _bounds: Rect, _content: Size) -> Point {
        state.view.pan
    }

    fn build(&self, state: &mut NodeCanvasState) -> Vec<Element> {
        let geometry = state.geometry(&self.graph);
        let zoom = state.view.zoom;
        let hover = state.hover;
        let target = match &state.drag {
            Some(Drag::Link { target, .. }) => *target,
            _ => None,
        };
        let mut children = Vec::with_capacity(self.graph.nodes.len() + self.graph.frames.len() + 2);
        for frame in &self.graph.frames {
            let Some(rect) = geometry.frame_rect(frame.key) else {
                continue;
            };
            let moved = match (&state.drag, &frame.area) {
                (Some(Drag::Frame { key, by, .. }), FrameArea::At { .. }) if *key == frame.key => {
                    rect.translate(*by)
                }
                (Some(Drag::Resize { key, by, .. }), _) if *key == frame.key => {
                    Rect::from_origin_size(
                        rect.origin,
                        Size::new(
                            (rect.width() + by.x).max(0.0),
                            (rect.height() + by.y).max(0.0),
                        ),
                    )
                }
                _ => rect,
            };
            let layer = Frame {
                frame: frame.clone(),
                rect: moved,
                zoom,
                selected: self.selection.contains(Item::Frame(frame.key)),
            };
            let placed = layer.placed();
            children.push(
                Element::new(layer)
                    .position(placed.x(), placed.y())
                    .keyed(("frame", frame.key.0)),
            );
        }
        let hidden = match &state.drag {
            Some(Drag::Link {
                detached: Some(detached),
                fixed,
                ..
            }) => Some(CanvasLink::new(*fixed, *detached)),
            _ => None,
        };
        let lit: Vec<CanvasLink> = self
            .graph
            .links
            .iter()
            .filter(|link| {
                self.selection.contains(Item::Link(**link))
                    || hover == Some(Hit::Link(**link))
                    || self.selection.contains(Item::Node(link.from.node))
                    || self.selection.contains(Item::Node(link.to.node))
            })
            .copied()
            .collect();
        let area = Wires::area(&geometry, zoom);
        children.push(
            Element::new(Wires {
                graph: Arc::clone(&self.graph),
                geometry: Arc::clone(&geometry),
                zoom,
                area,
                lit,
                hidden,
            })
            .position(area.x(), area.y())
            .keyed("wires"),
        );
        let wiring = wiring(&self.graph);
        for ((at, node), wired) in self.graph.nodes.iter().enumerate().zip(wiring) {
            let Some(rect) = geometry.node_rect(node.key) else {
                continue;
            };
            let hot = match (target, hover) {
                (Some(port), _) if port.node == node.key => Some((port.side, port.index)),
                (None, Some(Hit::Port(port))) if port.node == node.key => {
                    Some((port.side, port.index))
                }
                _ => None,
            };
            let card = Card {
                graph: Arc::clone(&self.graph),
                at,
                zoom,
                lit: Lit {
                    selected: self.selection.contains(Item::Node(node.key)),
                    focused: state.focus == Some(node.key),
                    hovered: hover == Some(Hit::Node(node.key)),
                    hot,
                },
                wired_inputs: wired.inputs,
                wired_outputs: wired.outputs,
            };
            children.push(
                Element::new(card)
                    .position(rect.x() * zoom, rect.y() * zoom)
                    .keyed(("node", node.key.0)),
            );
        }
        let sketch = match &state.drag {
            Some(Drag::Marquee { from, to, .. }) if from != to => {
                Some(Sketch::Marquee(Rect::from_origin_size(
                    from.min(*to),
                    Size::new((to.x - from.x).abs(), (to.y - from.y).abs()),
                )))
            }
            Some(Drag::Link {
                fixed,
                pointer,
                target,
                ..
            }) => geometry.anchor(*fixed).map(|anchor| {
                let end = target
                    .and_then(|port| geometry.anchor(port))
                    .unwrap_or(*pointer);
                let color = self
                    .graph
                    .nodes
                    .iter()
                    .find(|node| node.key == fixed.node)
                    .and_then(|node| node.port(fixed.side, fixed.index))
                    .map_or(palette().accent, |port| port.color);
                match fixed.side {
                    PortSide::Output => Sketch::Link {
                        from: anchor,
                        to: end,
                        color,
                    },
                    PortSide::Input => Sketch::Link {
                        from: end,
                        to: anchor,
                        color,
                    },
                }
            }),
            _ => None,
        };
        if let Some(sketch) = sketch {
            let area = sketch.area(zoom);
            children.push(
                Element::new(Overlay { sketch, zoom })
                    .position(area.x(), area.y())
                    .keyed("sketch"),
            );
        }
        children
    }

    fn arranged(&self, state: &mut NodeCanvasState, arranged: Arranged<'_>) -> bool {
        state.room = arranged.room().size;
        if state.framed || state.room.width <= 0.0 || state.room.height <= 0.0 {
            return false;
        }
        state.framed = true;
        self.frame_all(state);
        true
    }

    fn paint(&self, state: &mut NodeCanvasState, ctx: &mut PaintCtx<'_>) {
        let bounds = ctx.bounds();
        let palette = palette();
        ctx.draw_quad(Quad::new(bounds).fill(palette.background));
        let spacing = GRID * state.view.zoom;
        let every = match spacing < SPARSEST_GRID {
            true => MAJOR_EVERY,
            false => 1,
        };
        let step = spacing * every as f32;
        let minor = palette.outline.scale_alpha(0.25);
        let major = palette.outline.scale_alpha(0.55);
        let first = |pan: f32| (-pan / step).floor() as i32;
        let start_x = first(state.view.pan.x);
        let start_y = first(state.view.pan.y);
        let columns = (bounds.width() / step).ceil() as i32 + 1;
        let rows = (bounds.height() / step).ceil() as i32 + 1;
        for column in start_x..start_x + columns {
            let x = bounds.x() + state.view.pan.x + column as f32 * step;
            let tint = match (column * every) % MAJOR_EVERY == 0 {
                true => major,
                false => minor,
            };
            ctx.draw_quad(
                Quad::new(Rect::new(x.round(), bounds.y(), 1.0, bounds.height())).fill(tint),
            );
        }
        for row in start_y..start_y + rows {
            let y = bounds.y() + state.view.pan.y + row as f32 * step;
            let tint = match (row * every) % MAJOR_EVERY == 0 {
                true => major,
                false => minor,
            };
            ctx.draw_quad(
                Quad::new(Rect::new(bounds.x(), y.round(), bounds.width(), 1.0)).fill(tint),
            );
        }
        if ctx.focused() {
            ctx.draw_quad(
                Quad::new(bounds.inflate(-1.0)).stroke(2.0, palette.accent.scale_alpha(0.6)),
            );
        }
    }

    fn event(&self, state: &mut NodeCanvasState, event: &Event, ctx: &mut EventCtx<'_>) {
        let bounds = ctx.bounds();
        match event {
            Event::PointerPressed {
                position,
                button,
                modifiers,
            } if bounds.contains(*position) || ctx.captured() => {
                ctx.request_focus();
                let local = Self::local(bounds, *position);
                self.press(state, local, *button, *modifiers, ctx);
            }
            Event::PointerMoved(position) => {
                let local = Self::local(bounds, *position);
                match state.drag {
                    Some(_) => {
                        state.pointer = Some(local);
                        self.drag_to(state, local, ctx);
                    }
                    None => self.hover(state, local, ctx),
                }
            }
            Event::PointerReleased { .. } => self.release(state, ctx),
            Event::PointerLeft | Event::PointerExited if state.drag.is_none() => {
                state.pointer = None;
                if state.hover.take().is_some() {
                    ctx.handle().rebuild();
                }
            }
            Event::Scroll {
                position,
                delta,
                modifiers,
                source,
            } => {
                let local = Self::local(bounds, *position);
                self.scroll(state, local, *delta, *modifiers, *source, ctx);
                ctx.set_handled();
            }
            Event::Gesture {
                position,
                gesture: Gesture::Pinch { scale, delta, .. },
            } => {
                let local = Self::local(bounds, *position);
                let view = state.view.panned(*delta).zoomed_at(local, scale.max(0.01));
                self.set_view(state, view, ctx);
                ctx.set_handled();
            }
            Event::KeyPressed { key, modifiers, .. } if ctx.focused() => {
                if self.key(state, key, *modifiers, ctx) {
                    ctx.set_handled();
                }
            }
            Event::Command(command) if ctx.focused() => {
                if self.command(state, *command, ctx) {
                    ctx.set_handled();
                }
            }
            Event::FocusGained | Event::FocusLost => {
                if state.focus.is_none() {
                    let geometry = state.geometry(&self.graph);
                    state.focus = geometry.first();
                }
                ctx.handle().rebuild();
                ctx.request_paint();
            }
            _ => {}
        }
    }

    fn cursor(&self, state: &NodeCanvasState, _bounds: Rect, _point: Point) -> Option<CursorShape> {
        Some(match (&state.drag, state.hover) {
            (Some(Drag::Pan { .. }), _) => CursorShape::Grabbing,
            (Some(Drag::Nodes { .. } | Drag::Frame { .. }), _) => CursorShape::Move,
            (Some(Drag::Resize { .. }), _) | (None, Some(Hit::FrameGrip(_))) => {
                CursorShape::ResizeNorthWestSouthEast
            }
            (Some(Drag::Link { .. }), _) | (None, Some(Hit::Port(_))) => CursorShape::Crosshair,
            (None, Some(Hit::Node(_) | Hit::FrameTitle(_))) => CursorShape::Grab,
            (None, Some(Hit::Link(_))) => CursorShape::Pointer,
            _ => CursorShape::Default,
        })
    }

    fn tooltip_at(
        &self,
        state: &NodeCanvasState,
        bounds: Rect,
        point: Point,
    ) -> Option<(Tooltip, Rect)> {
        let geometry = &state.geometry.as_ref()?.geometry;
        let local = point - bounds.origin;
        let Hit::Node(key) = geometry.hit(&self.graph, &state.view, local) else {
            return None;
        };
        let node = self.graph.nodes.iter().find(|node| node.key == key)?;
        let problem = node.problem.as_ref()?;
        let rect = geometry.node_rect(key)?;
        let badge = badge_area(node, state.view.zoom)
            .translate(bounds.origin + state.view.to_local(rect.origin));
        badge
            .inflate(4.0)
            .contains(point)
            .then(|| (Tooltip::new(problem.clone()), badge))
    }

    fn same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.graph, &other.graph)
            && self.selection == other.selection
            && self.on_select == other.on_select
            && self.on_edit == other.on_edit
            && self.on_menu == other.on_menu
            && self.on_activate == other.on_activate
    }

    fn debug_name(&self) -> &'static str {
        "NodeCanvas"
    }
}
