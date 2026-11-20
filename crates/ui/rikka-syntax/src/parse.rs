use std::ops::{ControlFlow, Range};
use std::time::{Duration, Instant};

use rikka_text::TextSpan;
use rikka_text_document::{Content, Splice};
use rikka_theme::{Syntax, Token};
use tree_sitter as ts;

use crate::injection::InjectionLayers;
use crate::language::Language;
use crate::lexicon::Lexicon;
use crate::region::Region;
use crate::rope::{chunk_at, edit_of};

const REPARSE_BUDGET: Duration = Duration::from_millis(8);

pub struct Parsed {
    language: Language,
    parser: ts::Parser,
    tree: Option<ts::Tree>,
    document: Option<u64>,
    edited: Option<u64>,
    parsed: Option<u64>,
    parsing: Option<u64>,
    injected: InjectionLayers,
}

impl std::fmt::Debug for Parsed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Parsed({:?})", self.language)
    }
}

impl Parsed {
    pub fn new(language: Language) -> Option<Parsed> {
        let rules = language.rules()?;
        let mut parser = ts::Parser::new();
        parser.set_language(&rules.language).ok()?;
        Some(Parsed {
            language,
            parser,
            tree: None,
            document: None,
            edited: None,
            parsed: None,
            parsing: None,
            injected: InjectionLayers::default(),
        })
    }

    pub fn language(&self) -> Language {
        self.language
    }

    pub fn read(&mut self, content: &Content, budget: Duration) -> bool {
        if self.document != Some(content.id()) {
            self.reset(content.id());
        }
        let revision = content.revision();
        if self.parsed == Some(revision) {
            return true;
        }
        self.apply_changes(content, revision);

        let until = Instant::now() + budget;
        let mut spent = |_: &ts::ParseState| match Instant::now() < until {
            true => ControlFlow::Continue(()),
            false => ControlFlow::Break(()),
        };
        let rope = content.rope();
        let read = self.parser.parse_with_options(
            &mut |byte, _| chunk_at(rope, byte),
            self.tree.as_ref(),
            Some(ts::ParseOptions::new().progress_callback(&mut spent)),
        );
        match read {
            Some(tree) => {
                self.tree = Some(tree);
                self.parsed = Some(revision);
                self.parsing = None;
                true
            }
            None => {
                self.parsing = Some(revision);
                false
            }
        }
    }

    pub fn spans(
        &mut self,
        content: &Content,
        within: Range<usize>,
        colors: Syntax,
    ) -> Vec<TextSpan> {
        let mut regions = Vec::new();
        self.regions(content, within, &mut regions);
        crate::spans_of(&regions, colors)
    }

    pub fn lexicon(&mut self, content: &Content, at: usize) -> Lexicon {
        let current = self.read(content, REPARSE_BUDGET);
        match self.tree.as_ref().filter(|_| current) {
            Some(tree) => Lexicon::from_tree(tree.root_node(), content.rope(), at, self.language),
            None => Lexicon::from_text(content.rope(), at, Some(self.language)),
        }
    }

    pub fn token_at(&mut self, content: &Content, at: usize) -> Option<Token> {
        if at == 0 || at > content.len() || !self.read(content, REPARSE_BUDGET) {
            return None;
        }
        let before = at - 1;
        let line = content.line_range(content.line_of(before));
        let mut regions = Vec::new();
        self.regions(content, line, &mut regions);
        regions
            .iter()
            .find(|region| region.range.contains(&before))
            .map(|region| region.token)
    }

    fn regions(&mut self, content: &Content, within: Range<usize>, into: &mut Vec<Region>) {
        let (Some(tree), Some(rules)) = (self.tree.as_ref(), self.language.rules()) else {
            return;
        };
        crate::read(
            rules,
            tree.root_node(),
            content.rope(),
            within,
            &mut self.injected,
            into,
        );
    }

    fn reset(&mut self, document: u64) {
        self.document = Some(document);
        self.tree = None;
        self.edited = None;
        self.parsed = None;
        self.injected.clear();
        if self.parsing.take().is_some() {
            self.parser.reset();
        }
    }

    fn apply_changes(&mut self, content: &Content, revision: u64) {
        if self.edited == Some(revision) {
            return;
        }
        let known = self.edited.filter(|_| self.tree.is_some());
        let splices: Option<Vec<Splice>> = known
            .and_then(|known| content.changes_since(known))
            .map(Iterator::collect);
        match splices {
            Some(splices) => {
                for splice in &splices {
                    if let Some(tree) = self.tree.as_mut() {
                        tree.edit(&edit_of(splice));
                    }
                    self.injected.apply_splice(splice);
                }
            }
            None => {
                self.tree = None;
                self.injected.clear();
            }
        }
        self.edited = Some(revision);
        if self.parsing.take().is_some() {
            self.parser.reset();
        }
    }
}
