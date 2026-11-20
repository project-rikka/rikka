use std::collections::BTreeSet;
use std::sync::Arc;

use cut_project::{KeyRef, Property};
use rikka_app::prelude::*;

use super::graph::hue;
use super::outline::{LANE, Outline, SUMMARY};
use crate::cut::Cut;

const NAME: f32 = 11.5;
const NAME_LINE: f32 = 16.0;
const INDENT: f32 = 14.0;
const DOT: f32 = 7.0;
const DOT_GAP: f32 = 6.0;
const PAD: f32 = 6.0;
const LIT: f32 = 0.18;
const LINE: f32 = 1.0;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Name {
    pub(crate) property: Property,
    pub(crate) label: String,
    pub(crate) keys: Vec<KeyRef>,
}

#[derive(Debug)]
pub(crate) struct Names {
    pub(crate) outline: Outline,
    pub(crate) names: Vec<Name>,
    pub(crate) picked: Arc<BTreeSet<KeyRef>>,
    pub(crate) plotted: Arc<BTreeSet<Property>>,
}

impl Names {
    fn is_lit(&self, name: &Name) -> bool {
        match self.outline.graph {
            true => self.plotted.contains(&name.property),
            false => !name.keys.is_empty() && name.keys.iter().all(|key| self.picked.contains(key)),
        }
    }

    fn press(&self, index: usize, adding: bool, ctx: &mut EventCtx<'_>) {
        let Some(name) = self.names.get(index) else {
            return;
        };
        if self.outline.graph {
            let property = name.property.clone();
            ctx.run(move |cut: &mut Cut| cut.plot_property(property.clone(), adding));
            return;
        }
        let mut keys: BTreeSet<KeyRef> = name.keys.iter().cloned().collect();
        if adding {
            keys.extend(self.picked.iter().cloned());
        }
        let keys: Vec<KeyRef> = keys.into_iter().collect();
        ctx.run(move |cut: &mut Cut| cut.pick_keys(keys.clone()));
    }
}

impl Widget for Names {
    type State = ();

    fn paint(&self, _state: &mut (), ctx: &mut PaintCtx<'_>) {
        let palette = palette();
        let bounds = ctx.bounds();
        ctx.draw_quad(Quad::new(bounds).fill(palette.surface));
        let dimmed = self.outline.graph && !self.plotted.is_empty();
        for (index, name) in self.names.iter().enumerate() {
            let top = bounds.y() + Outline::lane_top(index) - SUMMARY;
            let row = Rect::new(bounds.x(), top, bounds.width(), LANE);
            let lit = self.is_lit(name);
            if lit {
                ctx.draw_quad(Quad::new(row).fill(palette.selection.scale_alpha(LIT)));
            }
            let mut left = bounds.x() + INDENT;
            if self.outline.graph {
                ctx.draw_quad(
                    Quad::new(Rect::from_center(
                        Point::new(left + DOT / 2.0, row.center().y),
                        Size::new(DOT, DOT),
                    ))
                    .fill(hue(index))
                    .radius(DOT / 2.0),
                );
                left += DOT + DOT_GAP;
            }
            let ink = match dimmed && !lit {
                true => palette.text_muted,
                false => palette.text,
            };
            let run = TextRun::new(name.label.clone(), NAME)
                .no_wrap()
                .ellipsized();
            ctx.draw_text(
                Rect::new(
                    left,
                    row.center().y - NAME_LINE / 2.0,
                    (row.max_x() - left - PAD).max(0.0),
                    NAME_LINE,
                ),
                run,
                ink,
            );
            ctx.draw_quad(
                Quad::new(Rect::new(row.x(), row.max_y() - LINE, row.width(), LINE))
                    .fill(palette.outline),
            );
        }
    }

    fn event(&self, _state: &mut (), event: &Event, ctx: &mut EventCtx<'_>) {
        let Event::PointerPressed {
            position,
            button: PointerButton::Primary,
            modifiers,
        } = event
        else {
            return;
        };
        let bounds = ctx.bounds();
        let Some(index) = self.outline.listed_at(position.y - bounds.y() + SUMMARY) else {
            return;
        };
        ctx.keep_focus();
        ctx.set_handled();
        self.press(index, modifiers.shift || modifiers.control, ctx);
    }

    fn same(&self, other: &Names) -> bool {
        self.outline == other.outline
            && self.names == other.names
            && self.picked == other.picked
            && self.plotted == other.plotted
    }
}

impl View for Names {
    fn build(self) -> Element {
        let height = self.outline.body();
        Element::new(self).height(height)
    }
}
