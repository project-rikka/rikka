use cut_project::{ClipId, Placement, Visual};
use rikka_app::prelude::*;
use rikka_figure::GRIP_REACH;
use rikka_figure_editing::paint_line;
use rikka_ui::DRAG_SLOP;

use super::Stage;
use super::item_projection::ItemProjection;
use super::items::{HIT_REACH, Item};
use super::viewing::Viewing;

const ARM: f32 = 9.0;
const RING: f32 = 9.0;
const CORE: f32 = 5.0;

#[derive(Clone, Debug)]
pub(crate) struct Anchoring {
    clip: ClipId,
    visual: Visual,
    projection: ItemProjection,
    offset: Point,
    origin: Point,
    moved: bool,
    reached: Visual,
}

impl Stage {
    pub(super) fn anchor_target<'a>(
        &self,
        items: &'a [Item],
        at: Point,
        position: Point,
        viewing: &Viewing,
    ) -> Option<&'a Item> {
        let crossed = self.selected_items(items).into_iter().find(|item| {
            item.anchor()
                .is_some_and(|spot| viewing.to_screen(spot).distance(position) <= GRIP_REACH)
        });
        crossed.or_else(|| self.hit(items, at, HIT_REACH / viewing.scale.max(f32::EPSILON)))
    }
}

impl Anchoring {
    pub(crate) fn grab(item: &Item, at: Point, position: Point) -> Option<Anchoring> {
        let projection = item.projection.clone()?;
        let pointer = projection.projection().unmap(at)?;
        Some(Anchoring {
            clip: item.clip,
            offset: item.visual.placement().anchor - pointer,
            visual: item.visual.clone(),
            reached: item.visual.clone(),
            projection,
            origin: position,
            moved: false,
        })
    }

    pub(crate) fn preview(&self) -> (ClipId, Visual) {
        (self.clip, self.reached.clone())
    }

    pub(crate) fn has_moved(&self) -> bool {
        self.moved
    }

    pub(crate) fn drag(&mut self, at: Point, position: Point) -> bool {
        if !self.moved && self.origin.distance(position) < DRAG_SLOP {
            return false;
        }
        self.moved = true;
        let Some(pointer) = self.projection.projection().unmap(at) else {
            return true;
        };
        let placement = self.visual.placement();
        let anchor = pointer + self.offset;
        let by = self
            .projection
            .placing()
            .apply_vector(anchor - placement.anchor);
        self.reached = self.visual.placed(
            Placement {
                anchor,
                ..placement
            }
            .slid(by),
        );
        true
    }
}

pub(crate) fn paint_anchor(ctx: &mut PaintCtx<'_>, viewing: &Viewing, item: &Item) {
    let Some(anchor) = item.anchor() else {
        return;
    };
    let spot = viewing.to_screen(anchor);
    let accent = palette().accent;
    paint_line(
        ctx,
        spot - Point::new(ARM, 0.0),
        spot + Point::new(ARM, 0.0),
        accent,
    );
    paint_line(
        ctx,
        spot - Point::new(0.0, ARM),
        spot + Point::new(0.0, ARM),
        accent,
    );
    ctx.draw_circle(spot, RING, accent);
    ctx.draw_circle(spot, CORE, palette().surface);
}
