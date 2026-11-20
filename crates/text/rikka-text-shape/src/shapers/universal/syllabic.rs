use rikka_text_unicode::IndicSyllabicCategory;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Syllabic {
    Unicode(IndicSyllabicCategory),
    SymbolModifier,
    Hieroglyph,
    HieroglyphJoiner,
    HieroglyphMarkBegin,
    HieroglyphMarkEnd,
    HieroglyphMirror,
    HieroglyphModifier,
    HieroglyphSegmentBegin,
    HieroglyphSegmentEnd,
}
