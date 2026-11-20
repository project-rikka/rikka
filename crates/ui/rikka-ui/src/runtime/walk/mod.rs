mod paint;
mod visit;

use std::sync::Arc;
use std::time::Instant;

use rikka_animation::Motion;
use rikka_renderer::{GroupId, LayerId};
use rikka_types::{Color, CornerRadii, Point, Rect, RoundedRect, Size, WindowView};
use rustc_hash::FxHashMap;

use crate::ambient::ThemeHandle;
use crate::motion::Clock;
use crate::paint::{Fade, PaintedPrimitive, Revealing, Section, narrowed, on_grid};
use crate::runtime::handle::Marks;
use crate::runtime::layout::{Layouts, Node};
use crate::runtime::scene_sync::{Frame, Regrouping, Sink};
use crate::runtime::table::{PENDING, Table};
use crate::runtime::tips::Bubble;
use rikka_picture::Pulse;

pub(crate) const BLEED: f32 = 64.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Space {
    Group(Frame),
    Volatile,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Clip {
    pub(crate) shape: RoundedRect,
    pub(crate) space: Space,
}

impl Clip {
    #[inline]
    pub(crate) fn narrowed(outer: Option<Clip>, inner: Option<Clip>) -> Option<Clip> {
        match (outer, inner) {
            (outer, None) => outer,
            (None, inner) => inner,
            (Some(outer), Some(inner)) => Some(Clip::intersected(outer, inner)),
        }
    }

    #[inline(never)]
    fn intersected(outer: Clip, inner: Clip) -> Clip {
        let shared = match outer.space == inner.space {
            true => inner.space,
            false => Space::Volatile,
        };
        let (shape, space) = narrowed(
            (outer.shape, outer.space),
            (inner.shape, inner.space),
            shared,
        )
        .unwrap_or((RoundedRect::rect(Rect::ZERO), shared));
        Clip { shape, space }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Context {
    pub(crate) origin: Point,
    pub(crate) clip: Option<Clip>,
    pub(crate) loose: Option<Clip>,
    pub(crate) bounded: bool,
    pub(crate) layer: Option<LayerId>,
    pub(crate) scale: f32,
    pub(crate) offset: Point,
    pub(crate) frame: Frame,
    pub(crate) focus_within: bool,
    pub(crate) stack: u64,
}

impl Context {
    pub(crate) fn root(scale: f32) -> Self {
        Context {
            origin: Point::ZERO,
            clip: None,
            loose: None,
            bounded: true,
            layer: None,
            scale,
            offset: Point::ZERO,
            frame: Frame::default(),
            focus_within: false,
            stack: 0,
        }
    }

    pub(crate) fn carries(&self, last: &Context) -> bool {
        self.frame.group == last.frame.group
            && self.scale == last.scale
            && self.bounded == last.bounded
            && self.layer == last.layer
            && self.focus_within == last.focus_within
            && self.stack == last.stack
            && self.at() == last.at()
            && matches!((self.framed_loose(), last.framed_loose()), (Some(now), Some(was)) if now == was)
    }

    fn at(&self) -> (f32, f32) {
        let at = self.origin * self.scale + self.offset - self.frame.origin;
        (on_grid(at.x), on_grid(at.y))
    }

    fn framed_loose(&self) -> Option<Option<FramedClip>> {
        let Some(clip) = self.loose else {
            return Some(None);
        };
        let Space::Group(frame) = clip.space else {
            return None;
        };
        let shape = visit::physical(clip.shape, self.scale, self.offset);
        let rect = shape.rect.translate(Point::ZERO - frame.origin);
        Some(Some(FramedClip {
            group: frame.group,
            rect: [rect.x(), rect.y(), rect.width(), rect.height()].map(on_grid),
            radii: shape.radii,
        }))
    }
}

#[derive(PartialEq)]
struct FramedClip {
    group: Option<GroupId>,
    rect: [f32; 4],
    radii: CornerRadii,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct NodeGeometry {
    pub(crate) bounds: Rect,
    pub(crate) room: Rect,
    pub(crate) content: Size,
    pub(crate) viewport: Size,
    pub(crate) scale: f32,
    pub(crate) offset: Point,
    pub(crate) base: Option<Clip>,
    pub(crate) loose: Option<Clip>,
    pub(crate) bounded: bool,
    pub(crate) hovered: bool,
    pub(crate) focused: bool,
    pub(crate) focus_within: bool,
    pub(crate) background: Option<Color>,
    pub(crate) children: u64,
    pub(crate) frame: Frame,
    pub(crate) layer: Option<LayerId>,
    pub(crate) stack: u64,
}

#[derive(Debug)]
pub(crate) struct Outputs {
    pub(crate) subtree_opacity: f32,
    pub(crate) subtree_scale: Option<(f32, Point)>,
    pub(crate) children_clip: Option<RoundedRect>,
    pub(crate) glass: Vec<RoundedRect>,
    pub(crate) opaque: Vec<RoundedRect>,
    pub(crate) windows: Vec<WindowView>,
    pub(crate) grounded: bool,
    pub(crate) reads: u8,
    pub(crate) volatile: bool,
    pub(crate) topmost: bool,
    pub(crate) fades: Vec<Fade>,
    pub(crate) sections: Vec<Section>,
}

impl Default for Outputs {
    fn default() -> Self {
        Outputs {
            subtree_opacity: 1.0,
            subtree_scale: None,
            children_clip: None,
            glass: Vec::new(),
            opaque: Vec::new(),
            windows: Vec::new(),
            grounded: false,
            reads: 0,
            volatile: false,
            topmost: false,
            fades: Vec::new(),
            sections: Vec::new(),
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct WalkResult {
    pub(crate) active: Vec<u64>,
    pub(crate) wakes: Vec<(Instant, u64)>,
    pub(crate) view: bool,
    pub(crate) builds: Vec<Node>,
    pub(crate) prefetches: Vec<u64>,
    pub(crate) moved: bool,
    pub(crate) departed: Vec<Node>,
    pub(crate) hosting: Vec<(Node, bool)>,
    pub(crate) imaged: Vec<(Node, bool)>,
    pub(crate) shifted: Vec<Node>,
    pub(crate) pulses: Vec<Pulse>,
}

impl WalkResult {
    pub(crate) fn reset(&mut self) {
        self.active.clear();
        self.wakes.clear();
        self.view = false;
        self.builds.clear();
        self.prefetches.clear();
        self.moved = false;
        self.departed.clear();
        self.hosting.clear();
        self.imaged.clear();
        self.shifted.clear();
        self.pulses.clear();
    }
}

pub(crate) struct Walk<'a> {
    pub(crate) layouts: &'a Layouts,
    pub(crate) table: &'a mut Table,
    pub(crate) sink: Sink<'a>,
    pub(crate) theme: &'a ThemeHandle,
    pub(crate) marks: &'a Arc<Marks>,
    pub(crate) viewport: Size,
    pub(crate) focus: Option<u64>,
    pub(crate) clock: Clock,
    pub(crate) debut: Instant,
    pub(crate) transaction: Option<Motion>,
    pub(crate) pivots: &'a FxHashMap<Node, Point>,
    pub(crate) all: bool,
    pub(crate) spare: Spare,
    pub(crate) reveal: Option<Revealing>,
    pub(crate) bubble: Option<(Node, Bubble)>,
    pub(crate) out: WalkResult,
}

#[derive(Default)]
pub(crate) struct Spare {
    pub(crate) scratch: Vec<PaintedPrimitive>,
    pub(crate) fades: Vec<Fade>,
    pub(crate) sections: Vec<Section>,
    pub(crate) regrouping: Regrouping,
    open: Vec<Open>,
}

struct Open {
    end: u32,
    node: Node,
    within: Context,
    force: bool,
}

impl Walk<'_> {
    pub(crate) fn run(&mut self, scale: f32, force: bool) {
        let Some(island) = self.table.islands.root() else {
            return;
        };
        let mut open = std::mem::take(&mut self.spare.open);
        open.clear();
        self.island(island, Context::root(scale), force, &mut open);
        self.spare.open = open;
    }

    fn island(&mut self, island: u32, entry: Context, force: bool, open: &mut Vec<Open>) {
        let (nodes, end) = self.table.islands.take_order(island);
        let base = open.len();
        let mut at = 0usize;
        while at < nodes.len() {
            let node = nodes[at];
            self.prefetch_ahead(&nodes[at + 1..]);
            let (context, forced) = open[base..]
                .last()
                .map_or((entry, force), |top| (top.within, top.force));
            match self.visit(node, context, forced) {
                None => at = end[at] as usize,
                Some(entered) => {
                    if let Some(nested) = self.table.islands.owned(node) {
                        self.island(nested, entered.within, entered.force, open);
                        self.close(node, entered.within);
                    } else if end[at] as usize > at + 1 {
                        open.push(Open {
                            end: end[at],
                            node,
                            within: entered.within,
                            force: entered.force,
                        });
                    } else {
                        self.close(node, entered.within);
                    }
                    at += 1;
                }
            }
            while let Some(top) = open[base..].last() {
                if top.end as usize != at {
                    break;
                }
                let (node, within) = (top.node, top.within);
                open.pop();
                self.close(node, within);
            }
        }
        self.table.islands.restore_order(island, nodes, end);
    }

    fn prefetch_ahead(&self, ahead: &[Node]) {
        let pending = |node: Node| self.all || self.table.flags(node) & PENDING != 0;
        if let Some(&next) = ahead.first()
            && pending(next)
        {
            for slot in self.table.prims(next) {
                self.sink.scene.prefetch(slot.node);
            }
        }
        if let Some(&after) = ahead.get(1)
            && pending(after)
        {
            self.table.prefetch_prims(after);
        }
        if let Some(&further) = ahead.get(2)
            && pending(further)
        {
            self.table.prefetch(further);
        }
    }
}

pub(crate) struct EnterContext {
    pub(crate) within: Context,
    pub(crate) force: bool,
}
