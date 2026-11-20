use cut_project::{ClipId, Property};
use rikka_app::prelude::*;

use crate::cut::Cut;

const SIDE: f32 = 20.0;
const GLYPH: f32 = 14.0;
const RADIUS: f32 = 4.0;
const HOVER: f32 = 0.08;
const UNREACHABLE: f32 = 0.4;
const DIAMOND: &str = "cut-key";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mark {
    Here,
    Animated,
    Still,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum DiamondTarget {
    Group,
    Property(Property),
}

#[derive(Debug)]
pub(crate) struct Diamond {
    pub(crate) clip: ClipId,
    pub(crate) keyed: DiamondTarget,
    pub(crate) mark: Mark,
    pub(crate) reachable: bool,
}

impl Diamond {
    fn key(&self) -> (&'static str, ClipId, DiamondTarget) {
        (DIAMOND, self.clip, self.keyed.clone())
    }
}

impl Widget for Diamond {
    type State = ();

    fn paint(&self, _state: &mut (), ctx: &mut PaintCtx<'_>) {
        let palette = palette();
        let bounds = ctx.bounds();
        if ctx.hovered() && self.reachable {
            ctx.draw_quad(
                Quad::new(bounds)
                    .fill(palette.text.scale_alpha(HOVER))
                    .radius(RADIUS),
            );
        }
        let (fill, color) = match self.mark {
            Mark::Here => (1.0, palette.accent),
            Mark::Animated => (0.0, palette.accent),
            Mark::Still => (0.0, palette.text_muted),
        };
        let color = match self.reachable {
            true => color,
            false => color.scale_alpha(UNREACHABLE),
        };
        let glyph = Rect::from_center(bounds.center(), Size::new(GLYPH, GLYPH));
        ctx.draw_icon(
            glyph,
            IconGlyph::from(MaterialSymbol::DIAMOND)
                .spec(GLYPH)
                .fill(fill),
            color,
        );
    }

    fn event(&self, _state: &mut (), event: &Event, ctx: &mut EventCtx<'_>) {
        let Event::PointerPressed { button, .. } = event else {
            return;
        };
        let keyed = self.keyed.clone();
        match button {
            PointerButton::Primary if self.reachable => {
                ctx.set_handled();
                ctx.keep_focus();
                ctx.run(move |cut: &mut Cut| match keyed.clone() {
                    DiamondTarget::Group => cut.toggle_group(),
                    DiamondTarget::Property(property) => cut.toggle_key(property),
                });
            }
            PointerButton::Secondary => {
                ctx.set_handled();
                ctx.keep_focus();
                let anchor = Anchor::new(self.key(), AnchorEdge::BelowEnd);
                ctx.run(move |cut: &mut Cut| match keyed.clone() {
                    DiamondTarget::Group => cut.open_group_easing(anchor),
                    DiamondTarget::Property(property) => {
                        cut.open_key_easing(vec![property], anchor)
                    }
                });
            }
            _ => {}
        }
    }

    fn behavior(&self, _state: &()) -> Behavior {
        Behavior::TRACKS_HOVER
    }

    fn same(&self, other: &Diamond) -> bool {
        self.clip == other.clip
            && self.keyed == other.keyed
            && self.mark == other.mark
            && self.reachable == other.reachable
    }
}

impl View for Diamond {
    fn build(self) -> Element {
        let key = self.key();
        let tip = match (self.reachable, self.mark) {
            (false, _) => None,
            (true, Mark::Here) => Some(tr!("cut.key.remove")),
            (true, Mark::Animated | Mark::Still) => Some(tr!("cut.key.add")),
        };
        let diamond = Element::new(self).size(SIDE, SIDE).keyed(key);
        match tip {
            Some(tip) => diamond.tooltip(tip),
            None => diamond,
        }
    }
}
