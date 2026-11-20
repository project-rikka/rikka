use crate::edited::{Edited, Splice};
use crate::error::{Error, Result};
use crate::excerpt::Excerpt;

enum Edge {
    Above,
    Below,
}

struct Echo<'s> {
    splice: &'s Splice,
    side: Edge,
    line: usize,
}

impl Edited<'_> {
    pub(crate) fn check_echoes(&self) -> Result<()> {
        let blocks: Vec<String> = (0..self.splices.len())
            .filter_map(|index| self.echo_block(index))
            .collect();
        if blocks.is_empty() {
            return Ok(());
        }
        Err(Error::EchoedLine {
            path: self.path.to_owned(),
            report: blocks.join("\n\n"),
        })
    }

    fn echo_block(&self, index: usize) -> Option<String> {
        let echoes: Vec<Echo<'_>> = [self.above(index), self.below(index)]
            .into_iter()
            .flatten()
            .collect();
        if echoes.is_empty() {
            return None;
        }
        let old = std::slice::from_ref(&self.splices[index].old);
        let lines: Vec<String> = echoes
            .iter()
            .map(Echo::describe)
            .chain(std::iter::once(
                Excerpt::around(&self.before, old).to_string(),
            ))
            .chain(echoes.iter().map(Echo::suggestion))
            .collect();
        Some(lines.join("\n"))
    }

    fn above(&self, index: usize) -> Option<Echo<'_>> {
        let splice = &self.splices[index];
        let first = *self
            .after
            .get(splice.new.start)
            .filter(|_| !splice.new.is_empty())?;
        let neighbour = splice.old.start.checked_sub(1)?;
        let touched = index > 0 && self.splices[index - 1].old.end > neighbour;
        let unchanged =
            !splice.is_insertion() && same_trimmed(self.before[splice.old.start], first);
        (!touched && !unchanged && is_echo(self.before[neighbour], first)).then_some(Echo {
            splice,
            side: Edge::Above,
            line: neighbour + 1,
        })
    }

    fn below(&self, index: usize) -> Option<Echo<'_>> {
        let splice = &self.splices[index];
        let last = *self
            .after
            .get(splice.new.end.checked_sub(1)?)
            .filter(|_| !splice.new.is_empty())?;
        let neighbour = splice.old.end;
        let beside = *self.before.get(neighbour)?;
        let touched = self
            .splices
            .get(index + 1)
            .is_some_and(|next| next.old.start <= neighbour);
        let unchanged =
            !splice.is_insertion() && same_trimmed(self.before[splice.old.end - 1], last);
        (!touched && !unchanged && is_echo(beside, last)).then_some(Echo {
            splice,
            side: Edge::Below,
            line: neighbour + 1,
        })
    }
}

impl Echo<'_> {
    fn describe(&self) -> String {
        let (end, place) = match self.side {
            Edge::Above => ("begins", "above"),
            Edge::Below => ("ends", "below"),
        };
        let text = match self.splice.is_insertion() {
            true => format!("The text put before line {}", self.splice.old.start + 1),
            false => format!("The text for {}", self.splice.describe()),
        };
        format!(
            "{text} {end} with line {}, which stays {place} it:",
            self.line
        )
    }

    fn suggestion(&self) -> String {
        let line = self.line;
        match (self.splice.is_insertion(), &self.side) {
            (false, Edge::Above) => format!(
                "Leave line {line} out of the text, or start the range at {line} to rewrite it too."
            ),
            (false, Edge::Below) => format!(
                "Leave line {line} out of the text, or end the range at {line} to rewrite it too."
            ),
            (true, _) => format!(
                "Leave line {line} out of the text, or replace line {line} with the text if it should appear twice."
            ),
        }
    }
}

fn same_trimmed(one: &str, other: &str) -> bool {
    one.trim() == other.trim()
}

fn is_echo(unchanged: &str, written: &str) -> bool {
    same_trimmed(unchanged, written) && unchanged.chars().any(char::is_alphanumeric)
}
