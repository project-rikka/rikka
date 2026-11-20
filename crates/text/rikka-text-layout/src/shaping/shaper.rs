use std::cell::RefCell;
use std::ops::Range;
use std::sync::Arc;

use rikka_text_catalog::{FaceMatch, catalog};
use rikka_text_unicode::{BaseDirection, Bidi, Level, Script};

use crate::fonts::{FaceInstance, face_instance};
use crate::shaping::columns::{GlyphColumns, flag};
use crate::shaping::itemize::{Item, Itemizer};
use crate::shaping::paragraph::Paragraph;
use crate::shaping::run::{Run, RunStyle};
use crate::shaping::segments::{Segment, segments};
use crate::shaping::shape_run::{Context, RunRequest, shape_run};
use crate::shaping::shaped_paragraph::{Shaped, ShapedParagraph};
use crate::style::{Caps, StyleSpan};

#[derive(Default)]
struct Scratch {
    bidi: Bidi,
    items: Vec<Item>,
    spans: Vec<(Range<usize>, u16)>,
    styles: Vec<RunStyle>,
    primaries: Vec<Option<FaceMatch>>,
    matches: Vec<FaceMatch>,
    primary_matches: Vec<Option<u16>>,
    match_slots: Vec<Option<Option<u16>>>,
    faces: Vec<Arc<FaceInstance>>,
    glyphs: GlyphColumns,
    runs: Vec<Run>,
    segments: Vec<Segment>,
    equivalences: Vec<((u16, Script, Script, bool), bool)>,
    caps: Caps,
}

thread_local! {
    static SCRATCH: RefCell<Scratch> = RefCell::new(Scratch::default());
}

pub(crate) fn shape(paragraph: &Paragraph<'_>) -> ShapedParagraph {
    SCRATCH.with(|scratch| {
        let mut scratch = scratch.borrow_mut();
        ShapedParagraph(Arc::new(scratch.shape(paragraph)))
    })
}

impl Scratch {
    fn shape(&mut self, paragraph: &Paragraph<'_>) -> Shaped {
        let text = paragraph.text;
        self.caps = paragraph.style.caps;
        self.resolve_styles(paragraph);
        self.bidi.analyze(text, BaseDirection::Auto);
        let base_level = self
            .bidi
            .paragraphs()
            .first()
            .map_or(Level::LEFT_TO_RIGHT, |paragraph| paragraph.level);
        Itemizer {
            catalog: catalog(),
            text,
            levels: self.bidi.levels(),
            spans: &self.spans,
            primaries: &self.primaries,
        }
        .itemize(
            &mut self.items,
            &mut self.matches,
            &mut self.primary_matches,
        );
        self.faces.clear();
        self.match_slots.clear();
        self.match_slots.resize(self.matches.len(), None);
        self.glyphs.clear();
        self.runs.clear();
        self.equivalences.clear();
        self.place_primaries();
        let base_face = self.styles[0].primary;
        let mut first = 0;
        while first < self.items.len() {
            let item = self.items[first];
            let slot = item
                .face
                .and_then(|face| self.slot(face))
                .or(base_face)
                .or_else(|| (!self.faces.is_empty()).then_some(0));
            let mut last = first + 1;
            while last < self.items.len() && self.shapes_with(slot, item, self.items[last]) {
                last += 1;
            }
            if let Some(slot) = slot {
                self.shape_unit(text, first..last, slot);
            }
            first = last;
        }
        segments(
            text,
            &self.runs,
            &self.styles,
            &self.glyphs,
            &mut self.segments,
        );
        let bidi = base_level.is_rtl() || self.runs.iter().any(|run| run.level != base_level);
        let tabs = self.glyphs.flags.iter().any(|flags| flags & flag::TAB != 0);
        Shaped {
            text: text.into(),
            base_level,
            tab_width: paragraph.tab_width,
            caps: self.caps,
            faces: self.faces.as_slice().into(),
            styles: self.styles.as_slice().into(),
            runs: self.runs.as_slice().into(),
            glyphs: self.glyphs.clone(),
            segments: self.segments.as_slice().into(),
            bidi,
            tabs,
        }
    }

    fn resolve_styles(&mut self, paragraph: &Paragraph<'_>) {
        let catalog = catalog();
        let style = paragraph.style;
        let base = catalog.query(style.query());
        self.styles.clear();
        self.primaries.clear();
        self.spans.clear();
        self.styles.push(RunStyle::default());
        self.primaries.push(base);
        let mut kept_until = 0;
        for span in ordered(paragraph.spans) {
            let range = span.range.clone();
            let fits = range.start >= kept_until
                && range.start < range.end
                && range.end <= paragraph.text.len()
                && paragraph.text.is_char_boundary(range.start)
                && paragraph.text.is_char_boundary(range.end)
                && self.styles.len() < usize::from(u16::MAX);
            if !fits {
                continue;
            }
            kept_until = range.end;
            let primary = if span.changes_font() {
                catalog.query(&style.overridden(span.family.as_ref(), span.weight, span.style))
            } else {
                base
            };
            self.spans.push((range, self.styles.len() as u16));
            self.styles.push(RunStyle {
                size: span
                    .font_size
                    .filter(|size| size.is_finite() && *size > 0.0),
                underline: span.underline,
                strikethrough: span.strikethrough,
                primary: None,
            });
            self.primaries.push(primary);
        }
    }

    fn place_primaries(&mut self) {
        for style in 0..self.styles.len() {
            self.styles[style].primary =
                self.primary_matches[style].and_then(|face| self.slot(face));
        }
    }

    fn shapes_with(&mut self, slot: Option<u16>, first: Item, next: Item) -> bool {
        if next.face != first.face || next.level != first.level {
            return false;
        }
        if next.script == first.script {
            return true;
        }
        let Some(slot) = slot else {
            return false;
        };
        let key = (slot, first.script, next.script, first.level.is_rtl());
        if let Some(&(_, equivalences)) = self.equivalences.iter().find(|(known, _)| *known == key)
        {
            return equivalences;
        }
        let equivalences = self.faces[usize::from(slot)].shapes_equivalently(
            first.script,
            next.script,
            key.3,
            self.caps,
        );
        self.equivalences.push((key, equivalences));
        equivalences
    }

    fn slot(&mut self, face: u16) -> Option<u16> {
        if let Some(slot) = self.match_slots[usize::from(face)] {
            return slot;
        }
        let slot = face_instance(&self.matches[usize::from(face)]).map(|instance| {
            match self
                .faces
                .iter()
                .position(|cached| Arc::ptr_eq(cached, &instance))
            {
                Some(position) => position as u16,
                None => {
                    self.faces.push(instance);
                    (self.faces.len() - 1) as u16
                }
            }
        });
        self.match_slots[usize::from(face)] = Some(slot);
        slot
    }

    fn shape_unit(&mut self, text: &str, items: Range<usize>, slot: u16) {
        let unit = &self.items[items.clone()];
        let first = unit[0];
        let start = first.start as usize;
        let end = unit[unit.len() - 1].end as usize;
        let glyphs_start = self.glyphs.len();
        let face = &self.faces[usize::from(slot)];
        shape_run(
            &RunRequest {
                text,
                range: start..end,
                face,
                script: first.script,
                rtl: first.level.is_rtl(),
                context: Context::FULL,
                caps: self.caps,
            },
            &mut self.glyphs,
        );
        let glyphs_end = self.glyphs.len();
        let mut run = Run {
            text: first.start..first.end,
            glyphs: glyphs_start as u32..glyphs_end as u32,
            face: slot,
            style: first.style,
            level: first.level,
            script: first.script,
        };
        for item in &unit[1..] {
            if item.style == run.style {
                continue;
            }
            let at = self
                .glyphs
                .first_at_or_after(run.glyphs.start as usize..glyphs_end, item.start);
            if at == glyphs_end {
                break;
            }
            if at == run.glyphs.start as usize {
                run.style = item.style;
                continue;
            }
            let boundary = self.glyphs.clusters[at];
            self.runs.push(Run {
                text: run.text.start..boundary,
                glyphs: run.glyphs.start..at as u32,
                ..run.clone()
            });
            run.text.start = boundary;
            run.glyphs.start = at as u32;
            run.style = item.style;
        }
        run.text.end = end as u32;
        self.runs.push(run);
    }
}

fn ordered(spans: &[StyleSpan]) -> impl Iterator<Item = &StyleSpan> {
    let sorted = spans.is_sorted_by_key(|span| span.range.start);
    let mut order: Vec<&StyleSpan> = Vec::new();
    if !sorted {
        order.extend(spans);
        order.sort_by_key(|span| span.range.start);
    }
    let direct = sorted.then(|| spans.iter());
    direct.into_iter().flatten().chain(order)
}
