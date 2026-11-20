use std::time::Instant;

use rikka_types::{Point, Rect};

use crate::input::Event;
use crate::runtime::layout::Node;
use crate::runtime::table::{CULLED, PAINT};
use crate::runtime::tips::Aim;
use crate::runtime::tree::UiTree;
use crate::view::Tooltip;

impl UiTree {
    pub(crate) fn next_wake(&self) -> Option<Instant> {
        match (self.wake, self.tips.due()) {
            (Some(painted), Some(tipped)) => Some(painted.min(tipped)),
            (painted, tipped) => painted.or(tipped),
        }
    }

    pub(super) fn hush_tips(&mut self, event: &Event) {
        if matches!(
            event,
            Event::PointerPressed { .. }
                | Event::Scroll { .. }
                | Event::Gesture { .. }
                | Event::KeyPressed { .. }
                | Event::DragMoved { .. }
        ) {
            self.tips.dismiss();
        }
    }

    pub(super) fn point_tips(&mut self, moved: bool) {
        let target = self.cursor.and_then(|point| {
            self.hovered
                .path()
                .iter()
                .zip(self.hovered.ids())
                .rev()
                .find_map(|(node, id)| self.aim_within(*node, *id, point))
        });
        self.tips.point(target, moved, Instant::now());
    }

    pub(super) fn focus_tips(&mut self) {
        let target = self.focus.and_then(|id| self.path_to(id)).and_then(|path| {
            path.iter()
                .rev()
                .map(|node| self.table.get(*node))
                .find(|instance| instance.tooltip.is_some())
                .map(|instance| Aim {
                    id: instance.id,
                    part: None,
                })
        });
        self.tips.focus(target, Instant::now());
    }

    pub(super) fn settle_tips(&mut self, root: Node, now: Instant) -> bool {
        let mut tips = std::mem::take(&mut self.tips);
        let changed = tips.settle(now, |aim| self.tip_of(aim));
        self.tips = tips;
        if changed {
            self.table.mark(&self.layouts, root, PAINT);
        }
        changed
    }

    fn aim_within(&self, node: Node, id: u64, point: Point) -> Option<Aim> {
        let instance = self.table.get(node);
        if instance.tooltip.is_some() {
            return Some(Aim { id, part: None });
        }
        let bounds = self.table.bounds(&self.layouts, node);
        let (_, area) = instance.widget.tooltip_at(
            instance.state.as_ref(),
            bounds,
            point,
            &self.theme,
            &instance.env,
        )?;
        Some(Aim {
            id,
            part: Some(area.translate(Point::ZERO - bounds.origin)),
        })
    }

    fn tip_of(&self, aim: Aim) -> Option<(Tooltip, Rect)> {
        let node = self.live(aim.id)?;
        if self.table.has(node, CULLED) {
            return None;
        }
        let instance = self.table.get(node);
        let bounds = self.table.bounds(&self.layouts, node);
        let (tooltip, area) = match aim.part {
            None => (Tooltip::clone(instance.tooltip.as_deref()?), bounds),
            Some(part) => {
                let (tooltip, area) = instance.widget.tooltip_at(
                    instance.state.as_ref(),
                    bounds,
                    self.cursor?,
                    &self.theme,
                    &instance.env,
                )?;
                if area.translate(Point::ZERO - bounds.origin) != part {
                    return None;
                }
                (tooltip, area)
            }
        };
        let anchor = match self.table.clip(&self.layouts, node) {
            Some(clip) => area.intersection(&clip)?,
            None => area,
        };
        Some((tooltip, anchor))
    }
}
