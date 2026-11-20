use std::hash::{Hash, Hasher};
use std::ops::RangeInclusive;

use rikka_types::Size;
use rustc_hash::FxHasher;

use crate::text::{DisplayText, TextDocument, TextRun, elide};

#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum MeasureSpec {
    Fixed(Size),
    Shrinkable {
        least: f32,
        natural: Size,
    },
    Text {
        run: TextRun,
        inset: Size,
        least: Option<Size>,
    },
    Document(DocumentMeasure),
}

impl MeasureSpec {
    pub fn text(run: TextRun) -> Self {
        Self::Text {
            run,
            inset: Size::ZERO,
            least: None,
        }
    }

    pub fn document(
        document: &TextDocument,
        template: TextRun,
        inset: Size,
        heights: RangeInclusive<f32>,
    ) -> Self {
        Self::Document(DocumentMeasure {
            document: document.clone(),
            revision: document.revision(),
            template,
            inset,
            heights,
        })
    }
}

#[derive(Clone)]
pub struct DocumentMeasure {
    document: TextDocument,
    revision: u64,
    template: TextRun,
    inset: Size,
    heights: RangeInclusive<f32>,
}

impl DocumentMeasure {
    pub(crate) fn inset(&self) -> Size {
        self.inset
    }

    pub(crate) fn wraps(&self) -> bool {
        !elide::elides(&self.template)
    }

    pub(crate) fn least(&self) -> f32 {
        *self.heights.start()
    }

    pub(crate) fn size_at(&self, width: Option<f32>) -> Size {
        let line_height = self.template.line_height.max(1.0);
        let room = (*self.heights.end() - self.inset.height).max(0.0);
        let content = self.document.content();
        let shown = DisplayText::new(&content);
        let mut rows = 0usize;
        let mut widest: f32 = 0.0;
        let mut line = String::new();
        for index in 0..shown.line_count() {
            if rows as f32 * line_height > room {
                break;
            }
            line.clear();
            shown.line_into(index, &mut line);
            if line.is_empty() {
                rows += 1;
                continue;
            }
            let run = TextRun {
                content: line.as_str().into(),
                ..self.template.clone()
            };
            let laid = rikka_text::layout(run.as_shape(), limit(&run, width));
            rows += laid.row_count().max(1);
            widest = widest.max(laid.widest());
        }
        Size::new(
            widest.ceil() + self.inset.width,
            rows.max(1) as f32 * line_height + self.inset.height,
        )
    }
}

impl PartialEq for DocumentMeasure {
    fn eq(&self, other: &Self) -> bool {
        self.document.same(&other.document)
            && self.revision == other.revision
            && self.template == other.template
            && self.inset == other.inset
            && self.heights == other.heights
    }
}

impl std::fmt::Debug for DocumentMeasure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DocumentMeasure")
            .field("revision", &self.revision)
            .field("heights", &self.heights)
            .finish_non_exhaustive()
    }
}

pub fn measure_text(run: &TextRun, width_limit: Option<f32>) -> Size {
    if run.content.is_empty() {
        return Size::new(0.0, run.line_height);
    }
    rikka_text::layout(run.as_shape(), limit(run, width_limit)).size()
}

pub(crate) fn limit(run: &TextRun, width: Option<f32>) -> Option<f32> {
    width.filter(|_| !elide::elides(run))
}

pub fn text_width(run: &TextRun) -> f32 {
    if run.content.is_empty() {
        return 0.0;
    }
    rikka_text::layout(run.as_shape(), None).widest()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct MeasureKey(u64);

pub(crate) fn measure_key(run: &TextRun, width: Option<f32>) -> MeasureKey {
    let mut hasher = FxHasher::default();
    hasher.write_u64(run.content.digest());
    run.size.to_bits().hash(&mut hasher);
    run.line_height.to_bits().hash(&mut hasher);
    run.family.hash(&mut hasher);
    run.weight.hash(&mut hasher);
    run.style.hash(&mut hasher);
    run.wrap.hash(&mut hasher);
    run.align.hash(&mut hasher);
    run.tab_width.hash(&mut hasher);
    run.ellipsize.hash(&mut hasher);
    for span in run.spans.iter() {
        span.range.hash(&mut hasher);
        span.font_size.map(f32::to_bits).hash(&mut hasher);
        span.weight.hash(&mut hasher);
        span.style.hash(&mut hasher);
        span.family.hash(&mut hasher);
    }
    width.map(f32::to_bits).hash(&mut hasher);
    MeasureKey(hasher.finish())
}
