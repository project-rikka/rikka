use std::collections::HashMap;
use std::path::Path;

use rikka_grammars::{Depths, Grammar, SyntaxError};

use crate::brackets;
use crate::edited::Edited;
use crate::error::{Error, Result};

const MAX_LISTED: usize = 6;

enum SyntaxCheck {
    Grammar(Grammar),
    Brackets,
}

impl SyntaxCheck {
    fn for_path(path: &Path) -> Option<SyntaxCheck> {
        if let Some(grammar) =
            Grammar::for_path(path).filter(|grammar| grammar.language().is_some())
        {
            return Some(SyntaxCheck::Grammar(grammar));
        }
        brackets::bracket_problems(path).then_some(SyntaxCheck::Brackets)
    }
}

impl Edited<'_> {
    pub(crate) fn check_syntax(&self, path: &Path, before: &str, after: &str) -> Result<()> {
        let found = match SyntaxCheck::for_path(path) {
            Some(SyntaxCheck::Grammar(grammar)) => self.grammar_problems(grammar, before, after),
            Some(SyntaxCheck::Brackets) => bracket_problems(before, after),
            None => None,
        };
        let Some((what, problems)) = found else {
            return Ok(());
        };

        let listed: String = problems
            .iter()
            .take(MAX_LISTED)
            .map(|problem| format!("- {problem}\n"))
            .collect();
        let more = match problems.len().checked_sub(MAX_LISTED) {
            Some(hidden) if hidden > 0 => format!("- and {hidden} more\n"),
            _ => String::new(),
        };
        let report = format!(
            "The edit leaves {what}:\n{listed}{more}The changed places would read:\n{}\nThey read now:\n{}",
            self.excerpt_after(),
            self.excerpt_before()
        );
        Err(Error::BrokenSyntax {
            path: self.path.to_owned(),
            report,
        })
    }

    fn grammar_problems(
        &self,
        grammar: Grammar,
        before: &str,
        after: &str,
    ) -> Option<(String, Vec<String>)> {
        let after_faults = grammar.syntax_errors(after)?;
        if after_faults.is_empty() {
            return None;
        }
        let before_faults = grammar.syntax_errors(before)?;
        let mut carried: HashMap<(Option<&str>, usize), usize> = HashMap::new();
        for fault in &before_faults {
            if let Some(line) = self.shifted_line(fault.line() - 1) {
                *carried.entry((token(fault), line + 1)).or_default() += 1;
            }
        }
        let introduced: Vec<&SyntaxError> = after_faults
            .iter()
            .filter(
                |fault| match carried.get_mut(&(token(fault), fault.line())) {
                    Some(left) if *left > 0 => {
                        *left -= 1;
                        false
                    }
                    _ => true,
                },
            )
            .collect();
        let worse = before_faults.is_empty() || after_faults.len() > before_faults.len();
        (worse && !introduced.is_empty()).then(|| {
            let mut problems = grammar
                .depths(before)
                .zip(grammar.depths(after))
                .map(|(before, after)| unbalanced(before, after))
                .unwrap_or_default();
            problems.extend(introduced.into_iter().map(describe));
            (
                format!("{} that no longer parses", grammar.name()),
                problems,
            )
        })
    }
}

fn bracket_problems(before: &str, after: &str) -> Option<(String, Vec<String>)> {
    let problems = unbalanced(
        brackets::bracket_depths(before),
        brackets::bracket_depths(after),
    );
    (!problems.is_empty()).then(|| ("its brackets unbalanced".to_owned(), problems))
}

fn unbalanced(before: Depths, after: Depths) -> Vec<String> {
    if !before.balanced() {
        return Vec::new();
    }
    after
        .unbalanced()
        .map(|(pair, more)| match more > 0 {
            true => format!("{more} `{}` more than `{}`", pair.opens, pair.closes),
            false => format!("{} `{}` more than `{}`", -more, pair.closes, pair.opens),
        })
        .collect()
}

fn token(fault: &SyntaxError) -> Option<&str> {
    match fault {
        SyntaxError::Missing { token, .. } => Some(token),
        SyntaxError::Unparsed { .. } => None,
    }
}

fn describe(fault: &SyntaxError) -> String {
    match fault {
        SyntaxError::Missing { line, token } => format!("line {line}: missing `{token}`"),
        SyntaxError::Unparsed { lines } if lines.start() == lines.end() => {
            format!("line {} does not parse", lines.start())
        }
        SyntaxError::Unparsed { lines } => {
            format!("lines {}-{} do not parse", lines.start(), lines.end())
        }
    }
}
