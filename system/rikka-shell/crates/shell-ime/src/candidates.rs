use rikka_ime::{Candidate, CandidateLayout};
use rikka_theme::Corner;
use rikka_types::{Rect, Size};
use rikka_ui::motion::indicator::{self, Indicator};
use rikka_ui::text::TextRun;
use rikka_ui::{
    Axis, Element, Layout, Length, PaintCtx, Quad, Space, TextRole, View, Widget, measure_text,
    motion, theme,
};

const INSET: f32 = Space::TIGHT.px();

const BETWEEN: f32 = Space::HAIR.px();

const LABEL_GAP: f32 = Space::SNUG.px();

const LABEL_INK: f32 = 0.7;

const HIGHLIGHT: &str = "ime.highlight";

struct Entry {
    label: TextRun,
    text: TextRun,
    label_width: f32,
    size: Size,
    offset: f32,
}

pub(super) struct Candidates {
    entries: Vec<Entry>,
    focused: Option<usize>,
    along: Axis,
    size: Size,
}

pub(crate) fn candidates(
    entries: &[Candidate],
    focused: Option<usize>,
    layout: CandidateLayout,
) -> Candidates {
    let theme = theme();
    let body = theme.typography().body_size;
    let mut entries: Vec<Entry> = entries
        .iter()
        .map(|candidate| {
            let caption = body * TextRole::Caption.scale();
            let label = TextRun::new(candidate.label.clone(), caption).no_wrap();
            let text = TextRun::new(candidate.text.clone(), body).no_wrap();
            let (label_size, text_size) = (measure_text(&label, None), measure_text(&text, None));
            Entry {
                label_width: label_size.width,
                size: Size::new(
                    INSET * 2.0 + label_size.width + LABEL_GAP + text_size.width,
                    INSET * 2.0 + label_size.height.max(text_size.height),
                ),
                label,
                text,
                offset: 0.0,
            }
        })
        .collect();
    let along = match layout {
        CandidateLayout::Vertical => Axis::Vertical,
        CandidateLayout::Horizontal => Axis::Horizontal,
    };
    let mut at = 0.0;
    for entry in &mut entries {
        entry.offset = at;
        at += along.length(entry.size) + BETWEEN;
    }
    let reach = (at - BETWEEN).max(0.0);
    let breadth = entries
        .iter()
        .map(|entry| along.across().length(entry.size))
        .fold(0.0, f32::max);
    let size = match along {
        Axis::Vertical => Size::new(breadth, reach),
        Axis::Horizontal => Size::new(reach, breadth),
    };
    Candidates {
        entries,
        focused,
        along,
        size,
    }
}

impl Candidates {
    fn band(&self, bounds: Rect, at: f32, extent: f32) -> Rect {
        match self.along {
            Axis::Vertical => Rect::new(bounds.x(), bounds.y() + at, bounds.width(), extent),
            Axis::Horizontal => Rect::new(bounds.x() + at, bounds.y(), extent, bounds.height()),
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct CandidatesState {
    highlight: Indicator,
}

impl Widget for Candidates {
    type State = CandidatesState;

    fn layout(&self) -> Layout {
        let width = match self.along {
            Axis::Vertical => Length::FILL,
            Axis::Horizontal => Length::px(self.size.width),
        };
        Layout::row()
            .size(width, Length::px(self.size.height))
            .min_width(Length::px(self.size.width))
            .shrink(0.0)
    }

    fn paint(&self, state: &mut CandidatesState, ctx: &mut PaintCtx<'_>) {
        let bounds = ctx.bounds();
        let palette = ctx.theme().palette();
        let along = self.along;
        let focused = self
            .focused
            .and_then(|index| Some((index, self.entries.get(index)?)));
        let highlight = state.highlight.resolve(
            ctx,
            HIGHLIGHT,
            focused.map(|(index, entry)| (index, entry.offset)),
            palette.accent,
            motion::INDICATOR,
        );
        let span = state.highlight.extent(
            ctx,
            HIGHLIGHT,
            focused.map(|(_, entry)| along.length(entry.size)),
            motion::INDICATOR,
        );
        if let Some((at, fill)) = highlight {
            let pill = self.band(bounds, at, span);
            ctx.draw_quad(Quad::new(pill).fill(fill).radius(Corner::Item.radius()));
        }

        for entry in &self.entries {
            let extent = along.length(entry.size);
            let slot = self.band(bounds, entry.offset, extent);
            let emphasis =
                highlight.map_or(0.0, |(at, _)| indicator::emphasis(at, entry.offset, extent));
            let label_ink = palette
                .text_muted
                .mix(palette.on_accent.with_alpha(LABEL_INK), emphasis);
            let text_ink = palette.text.mix(palette.on_accent, emphasis);
            let x = slot.x() + INSET;
            let text_x = x + entry.label_width + LABEL_GAP;
            ctx.draw_text_line(slot, x, entry.label_width, entry.label.clone(), label_ink);
            ctx.draw_text_line(
                slot,
                text_x,
                (slot.max_x() - INSET - text_x).max(0.0),
                entry.text.clone(),
                text_ink,
            );
        }
    }
}

impl View for Candidates {
    fn build(self) -> Element {
        Element::new(self)
    }
}

impl std::fmt::Debug for Candidates {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Candidates")
            .field("entries", &self.entries.len())
            .field("focused", &self.focused)
            .finish_non_exhaustive()
    }
}
