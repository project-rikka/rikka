use rikka_text_unicode::{BaseDirection, Bidi, BidiClass, VisualLine, bidi_class};

pub(crate) struct Resolution {
    pub(crate) paragraph_level: u8,
    pub(crate) levels: Vec<Option<u8>>,
    pub(crate) order: Vec<usize>,
}

pub(crate) fn is_removed(class: BidiClass) -> bool {
    matches!(
        class,
        BidiClass::LeftToRightEmbedding
            | BidiClass::RightToLeftEmbedding
            | BidiClass::LeftToRightOverride
            | BidiClass::RightToLeftOverride
            | BidiClass::PopDirectionalFormat
            | BidiClass::BoundaryNeutral
    )
}

pub(crate) fn resolve(
    bidi: &mut Bidi,
    line: &mut VisualLine,
    text: &str,
    direction: BaseDirection,
) -> Resolution {
    bidi.analyze(text, direction);
    bidi.reorder_line(0..text.len(), line);
    let starts: Vec<usize> = text.char_indices().map(|(offset, _)| offset).collect();
    let levels = text
        .char_indices()
        .map(|(offset, character)| {
            (!is_removed(bidi_class(character))).then(|| line.levels()[offset].number())
        })
        .collect();
    let mut order = Vec::new();
    for run in line.runs() {
        let mut indices: Vec<usize> = starts
            .iter()
            .enumerate()
            .filter(|&(_, offset)| run.range.contains(offset))
            .map(|(index, _)| index)
            .collect();
        if run.level.is_rtl() {
            indices.reverse();
        }
        order.extend(indices.into_iter().filter(|&index| {
            let character = text[starts[index]..].chars().next().unwrap_or('\0');
            !is_removed(bidi_class(character))
        }));
    }
    Resolution {
        paragraph_level: bidi.paragraphs().first().map_or(0, |p| p.level.number()),
        levels,
        order,
    }
}

pub(crate) fn parse_levels(field: &str) -> Vec<Option<u8>> {
    field
        .split_whitespace()
        .map(|level| level.parse().ok())
        .collect()
}

pub(crate) fn parse_order(field: &str) -> Vec<usize> {
    field
        .split_whitespace()
        .map(|index| index.parse().unwrap_or(usize::MAX))
        .collect()
}
