use rikka_text::{TextLayout, TextSpan, Wrap};

use crate::paint::measure_text;
use crate::text::TextRun;

const ELLIPSIS: &str = "\u{2026}";

pub fn elides(run: &TextRun) -> bool {
    run.wrap == Wrap::None
}

pub fn least_width(run: &TextRun) -> Option<f32> {
    elides(run).then(|| {
        let mark = TextRun {
            content: ELLIPSIS.into(),
            spans: Default::default(),
            ..run.clone()
        };
        measure_text(&mark, None).width
    })
}

pub(crate) fn shown(run: TextRun, laid: &TextLayout) -> TextRun {
    if !laid.is_elided() {
        return run;
    }
    let shown = laid.visible_text();
    let visible_end = shown.find(ELLIPSIS).unwrap_or(shown.len());
    let spans: Vec<TextSpan> = run
        .spans
        .iter()
        .filter(|span| span.range.start < visible_end)
        .map(|span| TextSpan {
            range: span.range.start..span.range.end.min(visible_end),
            ..span.clone()
        })
        .collect();
    TextRun {
        content: shown.clone(),
        spans: spans.into(),
        ..run
    }
}
