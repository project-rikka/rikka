use std::iter::FusedIterator;
use std::ops::Range;

use crate::mirroring::paired_bracket;
use crate::properties::{BracketType, properties};
use crate::script::script_extensions;
use crate::tables::enums::Script;

const BRACKET_DEPTH: usize = 64;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ScriptRun {
    pub range: Range<usize>,
    pub script: Script,
}

#[derive(Clone, Copy, Debug)]
struct OpenBracket {
    closing: char,
    script: Script,
}

#[derive(Clone, Debug)]
pub struct ScriptRuns<'a> {
    text: &'a str,
    position: usize,
    brackets: [OpenBracket; BRACKET_DEPTH],
    depth: usize,
    unresolved: usize,
    carried: Option<Script>,
}

pub fn script_runs(text: &str) -> ScriptRuns<'_> {
    ScriptRuns {
        text,
        position: 0,
        brackets: [OpenBracket {
            closing: '\0',
            script: Script::Common,
        }; BRACKET_DEPTH],
        depth: 0,
        unresolved: 0,
        carried: None,
    }
}

impl ScriptRuns<'_> {
    fn open(&mut self, closing: char, script: Script) {
        if self.depth == BRACKET_DEPTH {
            self.brackets.copy_within(1.., 0);
            self.depth -= 1;
            self.unresolved = self.unresolved.min(self.depth);
        }
        self.brackets[self.depth] = OpenBracket { closing, script };
        self.depth += 1;
        if script == Script::Common {
            self.unresolved += 1;
        }
    }

    fn close(&mut self, closing: char) -> Option<Script> {
        let found = self.brackets[..self.depth]
            .iter()
            .rposition(|bracket| bracket.closing == closing)?;
        let script = self.brackets[found].script;
        self.depth = found;
        self.unresolved = self.unresolved.min(self.depth);
        Some(script)
    }

    fn resolve(&mut self, script: Script) {
        let start = self.depth - self.unresolved;
        for bracket in &mut self.brackets[start..self.depth] {
            bracket.script = script;
        }
        self.unresolved = 0;
    }
}

impl Iterator for ScriptRuns<'_> {
    type Item = ScriptRun;

    fn next(&mut self) -> Option<ScriptRun> {
        let start = self.position;
        if start >= self.text.len() {
            return None;
        }
        let (mut current, skip) = match self.carried.take() {
            Some(script) => (
                script,
                self.text[start..].chars().next().map_or(0, char::len_utf8),
            ),
            None => (Script::Common, 0),
        };
        for (offset, character) in self.text[start + skip..].char_indices() {
            let offset = offset + skip;
            let properties = properties(character);
            let mut script = properties.script();
            if matches!(script, Script::Common | Script::Inherited) {
                match properties.bracket_type() {
                    Some(BracketType::Open) => {
                        if let Some(bracket) = paired_bracket(character) {
                            self.open(bracket.pair, current);
                        }
                    }
                    Some(BracketType::Close) => {
                        if let Some(opening) = self.close(character) {
                            script = opening;
                        }
                    }
                    None => {}
                }
            }
            if matches!(script, Script::Common | Script::Inherited) || script == current {
                continue;
            }
            if current == Script::Common {
                current = script;
                self.resolve(script);
                continue;
            }
            if properties.has_script_extensions() && script_extensions(character).contains(&current)
            {
                continue;
            }
            self.position = start + offset;
            self.carried = Some(script);
            return Some(ScriptRun {
                range: start..start + offset,
                script: current,
            });
        }
        self.position = self.text.len();
        Some(ScriptRun {
            range: start..self.text.len(),
            script: current,
        })
    }
}

impl FusedIterator for ScriptRuns<'_> {}
