use std::time::Instant;

use rikka_renderer::Scene;
use rikka_types::{Point, Rect, Size};
use rustc_hash::{FxHashMap, FxHashSet};
use smallvec::SmallVec;

use crate::runtime::layout::{Fix, Layouts, Node};
use crate::runtime::table::{CULLED, PAINT, Table};
use crate::runtime::tree::UiTree;
use crate::style::Axis;
use crate::view::{Anchor, AnchorWidth, Arranged, State};

const REARRANGED: usize = 3;

#[derive(Default)]
pub(crate) struct AnchorPass {
    anchored: Vec<AnchoredNode>,
    planned: bool,
    pub(crate) pivots: FxHashMap<Node, Point>,
}

impl AnchorPass {
    fn clear(&mut self) {
        self.anchored.clear();
        self.planned = false;
        self.pivots.clear();
    }
}

struct AnchoredNode {
    node: Node,
    anchor: Anchor,
    target: Node,
}

impl UiTree {
    pub(super) fn arrange(&mut self, scene: &mut Scene, state: &mut dyn State) {
        self.arrange_all(scene, state);
        self.scroll_anchors.clear();
    }

    fn arrange_all(&mut self, scene: &mut Scene, state: &mut dyn State) {
        let resized = std::mem::take(&mut self.resized);
        let Some(root) = self.root else {
            return;
        };
        self.refresh_lengths();
        let touched = self.take_touched();
        if resized || touched || self.layouts.needs_solve(root) {
            self.warm_measurements(resized);
            self.record_scroll_anchors(resized || touched);
            self.solve();
        }
        self.pin_anchors();
        for _ in 0..REARRANGED {
            if !self.notify_arranged() {
                break;
            }
            self.rebuild_asked(scene, state);
            if self.root.is_none() {
                return;
            }
            self.refresh_lengths();
            let touched = self.take_touched();
            self.warm_measurements(false);
            self.record_scroll_anchors(touched);
            self.solve();
            self.pin_anchors();
        }
    }

    fn take_touched(&mut self) -> bool {
        let touched = std::mem::take(&mut self.touched);
        if touched {
            self.anchors.planned = false;
        }
        touched
    }

    fn pin_anchors(&mut self) {
        if !self.has_anchors {
            self.anchors.clear();
            return;
        }
        if self.resolve_anchors() {
            self.solve();
        }
    }

    fn refresh_lengths(&mut self) {
        for &node in &self.scrollers {
            let instance = self.table.get(node);
            let length = instance.widget.content_length(instance.state.as_ref());
            self.layouts.set_length(node, length);
        }
    }

    fn notify_arranged(&mut self) -> bool {
        let mut nodes = std::mem::take(&mut self.arranging);
        nodes.clear();
        nodes.extend(self.scrollers.iter().copied());
        nodes.extend(self.arrangers.iter().copied());
        let mut children = std::mem::take(&mut self.arranged_rects);
        let mut rearranged = Vec::new();
        for &node in &nodes {
            if !self.layouts.take_arranged(node) {
                continue;
            }
            let rect = self.layouts.rect(node);
            let lane = self.layouts.lane(node);
            let anchor = self.scroll_anchors.take(&self.layouts, node);
            let instance = self.table.get_mut(node);
            let room = match instance.layout.scrolls {
                Some(Axis::Horizontal) => {
                    Rect::new(0.0, 0.0, rect.width(), (rect.height() - lane).max(0.0))
                }
                _ => Rect::new(0.0, 0.0, (rect.width() - lane).max(0.0), rect.height()),
            };
            children.clear();
            children.extend(
                self.layouts
                    .children(node)
                    .iter()
                    .map(|child| self.layouts.rect(*child)),
            );
            if instance.widget.arranged(
                instance.state.as_mut(),
                Arranged::new(room, &children, anchor),
            ) {
                instance.asked_build = true;
                rearranged.push(node);
            }
            if anchor.is_some() {
                self.table.mark(&self.layouts, node, PAINT);
            }
        }
        self.arranging = nodes;
        self.arranged_rects = children;
        for &node in &rearranged {
            self.wants_build_above(node);
        }
        !rearranged.is_empty()
    }

    fn solve(&mut self) {
        let Some(root) = self.root else {
            return;
        };
        let _span = tracing::trace_span!("rikka_ui::layout").entered();
        let started = Instant::now();
        self.layouts.solve(root, self.viewport, self.measured);
        crate::stats::laid_out(started.elapsed());
    }

    fn warm_measurements(&mut self, whole: bool) {
        let Some(root) = self.root else {
            return;
        };
        let _span = tracing::trace_span!("rikka_ui::shape").entered();
        let mut measurements = Vec::new();
        gather_measurements(
            &self.table,
            &mut self.layouts,
            root,
            whole,
            &mut measurements,
        );
        if !whole {
            for index in 0..self.layouts.boundaries().len() {
                let boundary = self.layouts.boundaries()[index];
                gather_measurements(
                    &self.table,
                    &mut self.layouts,
                    boundary,
                    false,
                    &mut measurements,
                );
            }
        }
        crate::text::shaping::warm(measurements);
    }

    fn resolve_anchors(&mut self) -> bool {
        let Some(root) = self.root else {
            return false;
        };
        if !self.anchors.planned {
            self.plan_anchors(root);
        }
        let Self {
            layouts,
            table,
            anchors,
            viewport,
            ..
        } = self;
        anchors.pivots.clear();
        let mut moved = false;
        for anchored in &anchors.anchored {
            let target = placed_bounds(table, layouts, root, anchored.target);
            let parent_origin = parent_origin(table, layouts, root, anchored.node);
            let size = layouts.rect(anchored.node).size;
            let placed = anchored.anchor.place(target, size, *viewport);
            let shown = Rect::from_origin_size(
                placed.origin,
                Size::new(
                    size.width,
                    placed
                        .max_height
                        .map_or(size.height, |room| size.height.min(room)),
                ),
            );
            if let Some(toward) = anchored.anchor.toward(target, placed.rose) {
                anchors
                    .pivots
                    .insert(anchored.node, shown.nearest(toward) - placed.origin);
            }
            let fix = Fix::Pinned {
                at: Point::new(
                    placed.origin.x - parent_origin.x,
                    placed.origin.y - parent_origin.y,
                ),
                width: (anchored.anchor.width() == AnchorWidth::MatchTarget
                    || anchored.anchor.covers())
                .then(|| target.width()),
                height: anchored.anchor.covers().then(|| target.height()),
                max_height: placed.max_height,
            };
            if layouts.fixed(anchored.node) != Some(fix) {
                layouts.fix(anchored.node, fix);
                moved = true;
            }
        }
        moved
    }

    fn plan_anchors(&mut self, root: Node) {
        let mut waiting = Vec::new();
        gather_anchored(&self.table, &self.layouts, root, &mut waiting);
        let mut targets = FxHashMap::default();
        if !waiting.is_empty() {
            let measurements: FxHashSet<u64> =
                waiting.iter().map(|(_, anchor)| anchor.target()).collect();
            find_targets(
                &self.table,
                &self.layouts,
                root,
                &measurements,
                &mut targets,
            );
        }
        self.anchors.anchored = waiting
            .into_iter()
            .filter_map(|(node, anchor)| {
                let target = *targets.get(&anchor.target())?;
                Some(AnchoredNode {
                    node,
                    anchor,
                    target,
                })
            })
            .collect();
        self.anchors.planned = true;
    }
}

fn gather_anchored(table: &Table, layouts: &Layouts, node: Node, into: &mut Vec<(Node, Anchor)>) {
    let instance = table.get(node);
    if !instance.singular {
        return;
    }
    if let Some(anchor) = instance.anchor {
        into.push((node, anchor));
    }
    for &child in layouts.children(node) {
        gather_anchored(table, layouts, child, into);
    }
}

fn find_targets(
    table: &Table,
    layouts: &Layouts,
    node: Node,
    measurements: &FxHashSet<u64>,
    into: &mut FxHashMap<u64, Node>,
) {
    if let Some(key) = table.get(node).key
        && measurements.contains(&key)
    {
        into.insert(key, node);
    }
    for &child in layouts.children(node) {
        find_targets(table, layouts, child, measurements, into);
    }
}

fn placed_bounds(table: &Table, layouts: &Layouts, root: Node, node: Node) -> Rect {
    layouts
        .rect(node)
        .translate(parent_origin(table, layouts, root, node))
}

fn parent_origin(table: &Table, layouts: &Layouts, root: Node, node: Node) -> Point {
    let mut ancestors: SmallVec<[Node; 32]> = SmallVec::new();
    let mut at = node;
    while at != root {
        let Some(parent) = layouts.parent(at) else {
            break;
        };
        ancestors.push(parent);
        at = parent;
    }
    ancestors
        .iter()
        .rev()
        .fold(Point::ZERO, |origin, &ancestor| {
            let instance = table.get(ancestor);
            let bounds = layouts.rect(ancestor).translate(origin);
            let offset = instance.widget.children_offset(
                instance.state.as_ref(),
                bounds,
                layouts.content(ancestor),
            );
            bounds.origin + offset
        })
}

fn gather_measurements(
    table: &Table,
    layouts: &mut Layouts,
    node: Node,
    whole: bool,
    measurements: &mut Vec<crate::text::shaping::ShapeRequest>,
) {
    if table.has(node, CULLED) || !(whole || layouts.is_dirty(node)) {
        return;
    }
    if let Some(leaf) = layouts.leaf_mut(node)
        && let Some(request) = leaf.prime()
    {
        measurements.push(request);
    }
    for index in 0..layouts.children(node).len() {
        let child = layouts.children(node)[index];
        gather_measurements(table, layouts, child, whole, measurements);
    }
}
