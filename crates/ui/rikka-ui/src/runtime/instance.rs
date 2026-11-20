use std::any::{Any, TypeId};
use std::rc::Rc;

use rikka_renderer::LayerId;
use rikka_types::{Point, Rect, RoundedRect, Size};

use crate::motion::{Motions, Passage};
use crate::runtime::holding::ComponentSlot;
use crate::runtime::layout::Node;
use crate::runtime::scene_sync::{GroupScroll, PrimSlot};
use crate::runtime::scratch::Recycled;
use crate::runtime::walk::{Context, NodeGeometry, Outputs};
use crate::style::{Axis, Layout, Style};
use crate::view::{Anchor, AnyWidget, Behavior, Env, Scoped, Store, Tooltip, Visual};

use crate::runtime::table::Bound;

#[repr(C)]
pub(crate) struct Instance {
    pub(crate) id: u64,
    pub(crate) widget: Box<dyn AnyWidget>,
    pub(crate) state: Box<dyn Any>,
    pub(crate) key: Option<u64>,
    pub(crate) branch: Option<u64>,
    pub(crate) component: Option<Box<ComponentSlot>>,
    pub(crate) locals: Vec<(TypeId, Store)>,
    pub(crate) scopes: Vec<Rc<Scoped>>,
    pub(crate) style: Option<Box<Style>>,
    pub(crate) visual: Option<Box<Visual>>,
    pub(crate) tooltip: Option<Rc<Tooltip>>,
    pub(crate) built_in: u64,
    pub(crate) memo: Option<(u64, u64)>,
    pub(crate) leaving: Option<Exit>,
    pub(crate) env: Env,
    pub(crate) node: Node,
    pub(crate) behavior: Behavior,
    pub(crate) builds: bool,
    pub(crate) wants_build: bool,
    pub(crate) asked_build: bool,
    pub(crate) claimed: bool,
    pub(crate) singular: bool,
    pub(crate) paints_offscreen: bool,
    pub(crate) shifts: bool,
    pub(crate) moves: bool,
    pub(crate) hosts: bool,
    pub(crate) imaged: bool,
    pub(crate) settled: Option<f32>,
    pub(crate) anchor: Option<Anchor>,
    pub(crate) last_context: Option<Context>,
    pub(crate) drawn: Option<NodeGeometry>,
    pub(crate) placed: Option<Rect>,
    pub(crate) lane: f32,
    pub(crate) content_size: Size,
    pub(crate) prims: Recycled<PrimSlot>,
    pub(crate) motions: Motions,
    pub(crate) outputs: Outputs,
    pub(crate) layout: Layout,
    pub(crate) frame_curve: Option<rikka_animation::Curve>,
    pub(crate) visual_fade: Option<LayerId>,
    pub(crate) subtree_fade: Option<LayerId>,
    pub(crate) fades: Vec<LayerId>,
    pub(crate) sections: Vec<GroupScroll>,
    pub(crate) passage: Option<Box<Passage>>,
    pub(crate) build_reads: Vec<u64>,
    pub(crate) build_stamp: u64,
    pub(crate) told: Point,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Exit {
    pub(crate) index: usize,
    pub(crate) done: bool,
    hold: Hold,
}

#[derive(Clone, Copy, Debug)]
enum Hold {
    Drawn(Point),
    Shift(Point),
}

impl Exit {
    pub(crate) fn new(index: usize, drawn: Option<Point>) -> Exit {
        Exit {
            index,
            done: false,
            hold: drawn.map_or(Hold::Shift(Point::ZERO), Hold::Drawn),
        }
    }

    pub(crate) fn shift(&mut self, origin: Point) -> Point {
        let shift = match self.hold {
            Hold::Drawn(drawn) => drawn - origin,
            Hold::Shift(shift) => shift,
        };
        self.hold = Hold::Shift(shift);
        shift
    }
}

impl Instance {
    pub(crate) fn refresh_behavior(&mut self) {
        self.behavior = match self.leaving {
            Some(_) => Behavior::INERT,
            None => self.widget.behavior(self.state.as_ref()),
        };
    }

    pub(crate) fn relocate(&mut self, origin: Point) {
        let by = origin - self.told;
        self.told = origin;
        if by != Point::ZERO {
            self.widget.relocated(self.state.as_mut(), by);
        }
    }

    pub(crate) fn identity(&self) -> TypeId {
        if let Some(component) = &self.component {
            return component.identity;
        }
        match self.locals.first() {
            Some((identity, _)) => *identity,
            None => self.widget.widget_type(),
        }
    }

    pub(crate) fn clips(&self) -> bool {
        self.behavior.has(Behavior::CLIPS)
            || self.visual.as_ref().is_some_and(|visual| visual.clipped)
    }

    pub(crate) fn clip_rect(&self, bounds: Rect) -> Option<RoundedRect> {
        self.clips().then(|| RoundedRect::rect(bounds))
    }

    pub(crate) fn room(&self, bounds: Rect) -> Rect {
        match self.layout.scrolls {
            Some(Axis::Horizontal) => Rect::new(
                bounds.x(),
                bounds.y(),
                bounds.width(),
                (bounds.height() - self.lane).max(0.0),
            ),
            _ => Rect::new(
                bounds.x(),
                bounds.y(),
                (bounds.width() - self.lane).max(0.0),
                bounds.height(),
            ),
        }
    }

    pub(crate) fn bound(&self) -> Bound {
        let placed = self.layout.scrolls.is_some()
            || self.anchor.is_some()
            || self.layout.inset.is_some()
            || self.leaving.is_some()
            || self.shifts
            || self.moves
            || self.behavior.has(Behavior::TRAVELS);
        if placed {
            return Bound::Always;
        }
        let fixed =
            self.layout.size.width.fixed().is_some() && self.layout.size.height.fixed().is_some();
        match fixed {
            true => Bound::Fixed,
            false => Bound::Open,
        }
    }

    pub(crate) fn offsets_children(&self) -> bool {
        self.layout.scrolls.is_some() || self.shifts
    }
}

impl std::fmt::Debug for Instance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct(self.widget.debug_name())
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}
