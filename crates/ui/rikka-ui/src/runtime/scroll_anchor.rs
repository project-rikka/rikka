use rikka_types::{Point, Rect};
use smallvec::SmallVec;

use crate::runtime::layout::{Layouts, Node};
use crate::runtime::table::Table;
use crate::runtime::tree::UiTree;
use crate::style::Axis;
use crate::view::ScrollAnchor;

#[derive(Default)]
pub(crate) struct ScrollAnchors {
    pressed: Option<Point>,
    anchors: Vec<AnchorRecord>,
}

struct AnchorRecord {
    scroller: Node,
    axis: Axis,
    path: Path,
    offset: f32,
    touched: bool,
}

type Path = SmallVec<[Node; 16]>;

#[derive(Clone, Copy)]
struct Probe {
    along: f32,
    across: Option<f32>,
    visible: f32,
}

impl ScrollAnchors {
    pub(super) fn pressed(&mut self, at: Point) {
        self.pressed = Some(at);
    }

    pub(super) fn clear(&mut self) {
        self.pressed = None;
        self.anchors.clear();
    }

    pub(super) fn take(&mut self, layouts: &Layouts, scroller: Node) -> Option<ScrollAnchor> {
        let index = self
            .anchors
            .iter()
            .position(|anchor| anchor.scroller == scroller)?;
        let anchor = self.anchors.swap_remove(index);
        Some(ScrollAnchor {
            was: anchor.offset,
            is: path_offset(layouts, anchor.axis, &anchor.path),
            touched: anchor.touched,
        })
    }
}

impl UiTree {
    pub(super) fn record_scroll_anchors(&mut self, whole: bool) {
        self.scroll_anchors.anchors.clear();
        let pressed = self.scroll_anchors.pressed;
        for &scroller in &self.scrollers {
            if !whole && !self.layouts.is_dirty(scroller) {
                continue;
            }
            let instance = self.table.get(scroller);
            let Some(axis) = instance.layout.scrolls else {
                continue;
            };
            let bounds = self.table.bounds(&self.layouts, scroller);
            let shift = instance.widget.children_offset(
                instance.state.as_ref(),
                bounds,
                self.layouts.content(scroller),
            );
            let under = |at: Point| {
                let local = at - bounds.origin - shift;
                Probe {
                    along: axis.coord(local),
                    across: Some(axis.across().coord(local)),
                    visible: 0.0,
                }
            };
            let top = Probe {
                along: -axis.coord(shift),
                across: None,
                visible: axis.room(bounds),
            };
            let probe = match pressed.filter(|at| bounds.contains(*at)) {
                Some(at) => under(at),
                None if top.along > 0.0 => top,
                None => continue,
            };
            let anchor = find_anchor(&self.layouts, &self.table, scroller, axis, probe);
            self.scroll_anchors.anchors.extend(anchor);
        }
    }
}

fn find_anchor(
    layouts: &Layouts,
    table: &Table,
    scroller: Node,
    axis: Axis,
    probe: Probe,
) -> Option<AnchorRecord> {
    let touched = probe.across.is_some();
    let mut crossed: Path = SmallVec::new();
    let mut started: Option<(usize, Node)> = None;
    let mut probe = probe;
    let mut at = scroller;
    loop {
        let (crossing, below) = child_under(layouts, table, at, axis, probe);
        if let Some((child, start)) = below
            && (crossing.is_none() || start - probe.along < probe.visible)
        {
            started = Some((crossed.len(), child));
        }
        let Some(child) = crossing else {
            break;
        };
        crossed.push(child);
        if table.get(child).layout.scrolls.is_some() {
            break;
        }
        let origin = layouts.rect(child).origin;
        probe = Probe {
            along: probe.along - axis.coord(origin),
            across: probe
                .across
                .map(|across| across - axis.across().coord(origin)),
            visible: probe.visible,
        };
        at = child;
    }
    let mut path = crossed;
    if let Some((depth, child)) = started
        && (!touched || path.is_empty())
    {
        path.truncate(depth);
        path.push(child);
    }
    (!path.is_empty()).then(|| AnchorRecord {
        scroller,
        axis,
        offset: path_offset(layouts, axis, &path),
        path,
        touched,
    })
}

fn child_under(
    layouts: &Layouts,
    table: &Table,
    parent: Node,
    axis: Axis,
    probe: Probe,
) -> (Option<Node>, Option<(Node, f32)>) {
    let mut below: Option<(Node, f32)> = None;
    let mut crossing: Option<Node> = None;
    for &child in layouts.children(parent) {
        if !layouts.was_laid(child) || layouts.fixed(child).is_some() || !table.holds(child) {
            continue;
        }
        let rect = layouts.rect(child);
        let (start, end) = span(axis, rect);
        if start > probe.along {
            if below.is_none_or(|(_, nearest)| start < nearest) {
                below = Some((child, start));
            }
            continue;
        }
        if end <= probe.along {
            continue;
        }
        let across = span(axis.across(), rect);
        match probe.across {
            Some(at) if across.0 <= at && at < across.1 => return (Some(child), below),
            _ => {
                crossing.get_or_insert(child);
            }
        }
    }
    (crossing, below)
}

fn span(axis: Axis, rect: Rect) -> (f32, f32) {
    let start = axis.coord(rect.origin);
    (start, start + axis.room(rect))
}

fn path_offset(layouts: &Layouts, axis: Axis, path: &[Node]) -> f32 {
    path.iter()
        .map(|node| axis.coord(layouts.rect(*node).origin))
        .sum()
}
