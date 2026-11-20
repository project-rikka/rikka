use super::engine::Engine;
use super::language::Language;
use super::{char_at, char_before};
use crate::properties::properties;
use crate::tables::enums::{GraphemeClusterBreak, LineBreakClass};

#[derive(Clone, Debug, Default)]
pub(crate) struct ComplexRun {
    end: usize,
    next_range: usize,
    engine: Option<Engine>,
}

#[inline]
pub(crate) fn is_complex(character: char) -> bool {
    properties(character).line_break_class() == LineBreakClass::ComplexContext
}

fn is_grapheme_boundary(text: &str, at: usize) -> bool {
    let after = char_at(text, at).map(|character| properties(character).grapheme_cluster_break());
    let before =
        char_before(text, at).map(|character| properties(character).grapheme_cluster_break());
    !matches!(
        after,
        Some(
            GraphemeClusterBreak::Extend
                | GraphemeClusterBreak::SpacingMark
                | GraphemeClusterBreak::Zwj
        )
    ) && before != Some(GraphemeClusterBreak::Prepend)
}

impl ComplexRun {
    pub(crate) fn from_start(text: &str, start: usize) -> Self {
        let end = text[start..]
            .char_indices()
            .find(|&(_, character)| !is_complex(character))
            .map_or(text.len(), |(offset, _)| start + offset);
        Self {
            end,
            next_range: start,
            engine: None,
        }
    }

    pub(crate) fn from_position(text: &str, position: usize) -> Self {
        let start = text[..position]
            .char_indices()
            .rev()
            .find(|&(_, character)| !is_complex(character))
            .map_or(0, |(offset, character)| offset + character.len_utf8());
        Self::from_start(text, start)
    }

    pub(crate) fn end(&self) -> usize {
        self.end
    }

    pub(crate) fn next_break(&mut self, text: &str) -> Option<usize> {
        loop {
            if let Some(engine) = &mut self.engine {
                while let Some(boundary) = engine.next_break(text) {
                    if is_grapheme_boundary(text, boundary) {
                        return Some(boundary);
                    }
                }
                self.engine = None;
            }
            self.engine = Some(self.next_engine(text)?);
        }
    }

    fn next_engine(&mut self, text: &str) -> Option<Engine> {
        while self.next_range < self.end {
            let start = self.next_range;
            let character = char_at(text, start)?;
            let script = properties(character).script();
            let range_end = text[start..self.end]
                .char_indices()
                .find(|&(_, character)| properties(character).script() != script)
                .map_or(self.end, |(offset, _)| start + offset);
            self.next_range = range_end;
            if let Some(language) = Language::from_script(script) {
                return Some(Engine::new(text, language, start, range_end));
            }
        }
        None
    }
}
