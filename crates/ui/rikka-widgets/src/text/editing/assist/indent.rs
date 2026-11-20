use rikka_ui::text::Content;

use crate::text::editing::assist::pairs::{after, closing};

const SAMPLED: usize = 2000;
const WIDEST: usize = 8;

pub(crate) fn unit_of(content: &Content, fallback: usize) -> String {
    let mut tabbed = 0usize;
    let mut spaced = 0usize;
    let mut steps = [0usize; WIDEST + 1];
    let mut previous = 0usize;
    let mut line = String::new();
    for index in 0..content.line_count().min(SAMPLED) {
        line.clear();
        content.line_into(index, &mut line);
        if line.trim().is_empty() {
            continue;
        }
        if line.starts_with('\t') {
            tabbed += 1;
            continue;
        }
        let width = line.len() - line.trim_start_matches(' ').len();
        if width > 0 {
            spaced += 1;
        }
        let step = width.abs_diff(previous);
        if (2..=WIDEST).contains(&step) {
            steps[step] += 1;
        }
        previous = width;
    }
    if tabbed > spaced {
        return "\t".to_owned();
    }
    let width = steps
        .iter()
        .enumerate()
        .filter(|(_, count)| **count > 0)
        .max_by_key(|(step, count)| (**count, std::cmp::Reverse(*step)))
        .map_or(fallback, |(step, _)| step);
    " ".repeat(width)
}

pub(crate) fn newline(content: &mut Content, unit: &str, openers: &str) -> bool {
    let selection = content.selection();
    let line_start = content.line_start(content.line_of(selection.start));
    let leading = content.slice(line_start..selection.start);
    let base: String = leading
        .chars()
        .take_while(|ch| *ch == ' ' || *ch == '\t')
        .collect();
    let last = leading.trim_end().chars().last();
    let opens = last.is_some_and(|last| closing(last).is_some() || openers.contains(last));
    let next = after(content, selection.end);
    let embraced = last
        .and_then(closing)
        .is_some_and(|close| next == Some(close));

    let mut inserted = format!("\n{base}");
    if opens {
        inserted.push_str(unit);
    }
    let caret = selection.start + inserted.len();
    if embraced {
        inserted.push('\n');
        inserted.push_str(&base);
    }
    let changed = content.replace_each(&[(selection, inserted)]);
    content.place_caret(caret, false);
    changed
}
