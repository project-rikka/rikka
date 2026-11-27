use crate::anchor::Anchor;
use crate::edited::{Edited, Splice};
use crate::error::{Error, Result};
use crate::excerpt::Excerpt;
use crate::placement::{Site, owner};

#[derive(Clone, Copy)]
enum Edge {
    Above,
    Below,
}

struct Echo<'s> {
    splice: &'s Splice,
    edge: Edge,
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
        let old = &echoes.first()?.splice.old;
        let lines: Vec<String> = echoes
            .iter()
            .map(|echo| self.describe(echo))
            .chain(std::iter::once(
                Excerpt::current(&self.before, std::slice::from_ref(old)).to_string(),
            ))
            .chain(echoes.iter().map(|echo| self.suggestion(echo)))
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
            edge: Edge::Above,
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
            edge: Edge::Below,
            line: neighbour + 1,
        })
    }

    fn describe(&self, echo: &Echo<'_>) -> String {
        let splice = echo.splice;
        let place = match (splice.site, splice.old.len()) {
            (Site::After, _) => format!("put after line {}", splice.old.start),
            (Site::Before, _) => format!("put before line {}", splice.old.start + 1),
            (Site::Lines, 1) => format!("for line {}", splice.old.start + 1),
            (Site::Lines, _) => format!("for lines {}-{}", splice.old.start + 1, splice.old.end),
        };
        let (verb, side) = match echo.edge {
            Edge::Above => ("begins", "above"),
            Edge::Below => ("ends", "below"),
        };
        format!(
            "{}text {place} {verb} with line {}, which stays {side} it:",
            owner(splice.edit, self.edits),
            echo.line
        )
    }

    fn suggestion(&self, echo: &Echo<'_>) -> String {
        let line = echo.line;
        let anchor = Anchor::of(line, self.before[line - 1]);
        match (echo.splice.is_insertion(), echo.edge) {
            (false, Edge::Above) => format!(
                "Leave line {line} out of the text, or start the range at `{anchor}` to rewrite it too."
            ),
            (false, Edge::Below) => format!(
                "Leave line {line} out of the text, or end the range at `{anchor}` to rewrite it too."
            ),
            (true, _) => format!(
                "Leave line {line} out of the text: an insertion keeps it. To repeat it on purpose, replace `{anchor}` with both copies."
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
