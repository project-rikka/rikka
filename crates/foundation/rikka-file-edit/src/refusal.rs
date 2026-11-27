use std::ops::Range;

use crate::excerpt::Excerpt;
use crate::matching::{self, Closest};
use crate::seam::{Echo, Edge};

const MAX_LISTED: usize = 8;

pub(crate) struct Place {
    index: usize,
    count: usize,
}

impl Place {
    pub(crate) fn new(index: usize, count: usize) -> Place {
        Place { index, count }
    }

    pub(crate) fn empty(&self) -> String {
        format!(
            "{}old_string is empty; it must be text the file holds.",
            self.owner()
        )
    }

    pub(crate) fn unchanged(&self) -> String {
        format!("{}old_string and new_string are the same.", self.owner())
    }

    pub(crate) fn no_match(&self, content: &str, old: &str) -> String {
        let lines: Vec<&str> = content.lines().collect();
        let mut report = vec![format!(
            "{}old_string does not appear in the file{}.",
            self.owner(),
            self.state()
        )];
        if matching::unnumbered(old).is_some_and(|unnumbered| content.contains(&unnumbered)) {
            report.push(
                "It still carries the line numbers shown before each line; leave them out."
                    .to_owned(),
            );
            return report.join("\n");
        }
        match matching::closest(&lines, old) {
            Some(Closest {
                lines: span,
                spacing_only,
            }) => {
                let at = describe(&span);
                report.push(match spacing_only {
                    true => format!("The closest text, at {at}, differs only in whitespace:"),
                    false => format!("The closest text is at {at}:"),
                });
                report.push(Excerpt::span(&lines, span).to_string());
                report.push(
                    "Copy old_string exactly from these lines, without their numbers.".to_owned(),
                );
            }
            None => report.push("Nothing in the file resembles it.".to_owned()),
        }
        report.join("\n")
    }

    pub(crate) fn ambiguous(&self, content: &str, found: &[usize]) -> String {
        let mut listed: Vec<String> = line_numbers(content, &found[..found.len().min(MAX_LISTED)])
            .iter()
            .map(usize::to_string)
            .collect();
        if found.len() > MAX_LISTED {
            listed.push(format!("{} more", found.len() - MAX_LISTED));
        }
        format!(
            "{}old_string appears {} times{}, at lines {}. Add surrounding lines to make it unique, or set replace_all to replace every one.",
            self.owner(),
            found.len(),
            self.state(),
            enumerate(&listed)
        )
    }

    pub(crate) fn echoed(&self, content: &str, echo: &Echo) -> String {
        let lines: Vec<&str> = content.lines().collect();
        let (verb, side) = match echo.edge {
            Edge::Above => ("begins", "above"),
            Edge::Below => ("ends", "below"),
        };
        format!(
            "{}new_string {verb} with line {}{}, which stays {side} the replaced text:\n{}\nLeave that line out of new_string, or include it in old_string to rewrite it too.",
            self.owner(),
            echo.line,
            self.state(),
            Excerpt::around(&lines, std::slice::from_ref(&echo.replaced))
        )
    }

    fn owner(&self) -> String {
        match self.count {
            1 => String::new(),
            _ => format!("Edit {}'s ", self.index + 1),
        }
    }

    fn state(&self) -> String {
        match self.index {
            0 => String::new(),
            1 => " after edit 1".to_owned(),
            last => format!(" after edits 1-{last}"),
        }
    }
}

fn describe(span: &Range<usize>) -> String {
    match span.len() {
        1 => format!("line {}", span.start + 1),
        _ => format!("lines {}-{}", span.start + 1, span.end),
    }
}

fn line_numbers(content: &str, found: &[usize]) -> Vec<usize> {
    let (mut line, mut from) = (1, 0);
    found
        .iter()
        .map(|&at| {
            line += matching::line_of(&content[from..], at - from);
            from = at;
            line
        })
        .collect()
}

fn enumerate(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [only] => only.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}
