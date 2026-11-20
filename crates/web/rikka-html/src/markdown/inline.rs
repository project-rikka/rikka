use super::longest_backtick_run;
use super::phrasing::{Phrasing, Piece, Style};

const ESCAPED: [char; 16] = [
    '\\', '`', '*', '_', '[', ']', '<', '>', '#', '|', '$', '^', '~', '&', '{', '}',
];
const LINE_START_ESCAPED: [char; 4] = ['-', '+', '=', ':'];
const MARKS: [Mark; 5] = [
    Mark::Link,
    Mark::Strike,
    Mark::Strong,
    Mark::Emphasis,
    Mark::Code,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Setting {
    Paragraph,
    Heading,
    Cell,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mark {
    Link,
    Strike,
    Strong,
    Emphasis,
    Code,
}

#[derive(Clone, Copy)]
struct Run<'a> {
    text: &'a str,
    style: &'a Style,
}

pub(super) fn render(phrasing: &Phrasing, setting: Setting) -> String {
    let separator = match setting {
        Setting::Paragraph => "\\\n",
        Setting::Heading | Setting::Cell => " ",
    };
    let mut rendered = String::new();
    for (index, line) in phrasing
        .pieces
        .split(|piece| matches!(piece, Piece::Break))
        .enumerate()
    {
        if index > 0 {
            rendered.push_str(separator);
        }
        let runs: Vec<Run<'_>> = line
            .iter()
            .filter_map(|piece| match piece {
                Piece::Text { text, style } => Some(Run { text, style }),
                Piece::Break => None,
            })
            .collect();
        let mut writer = Line {
            out: String::new(),
            setting,
        };
        writer.range(&runs, 0, None);
        rendered.push_str(&writer.out);
    }
    rendered
}

struct Line {
    out: String,
    setting: Setting,
}

impl Line {
    fn key<'a>(&self, mark: Mark, style: &'a Style) -> Option<&'a str> {
        let on = match mark {
            Mark::Link => return style.link.as_deref(),
            Mark::Strike => style.strike,
            Mark::Strong => style.strong,
            Mark::Emphasis => style.emphasis,
            Mark::Code => style.code && self.setting != Setting::Cell,
        };
        on.then_some("")
    }

    fn range(&mut self, runs: &[Run<'_>], level: usize, after: Option<char>) {
        let Some(&mark) = MARKS.get(level) else {
            for run in runs {
                self.escape(run.text);
            }
            return;
        };
        let mut start = 0;
        while start < runs.len() {
            let key = self.key(mark, runs[start].style);
            let end = runs[start..]
                .iter()
                .position(|run| self.key(mark, run.style) != key)
                .map_or(runs.len(), |offset| start + offset);
            let next = first_char(&runs[end..]).or(after);
            match key {
                Some(key) => self.group(&runs[start..end], level, mark, key, next),
                None => self.range(&runs[start..end], level + 1, next),
            }
            start = end;
        }
    }

    fn group(
        &mut self,
        runs: &[Run<'_>],
        level: usize,
        mark: Mark,
        key: &str,
        after: Option<char>,
    ) {
        let (lead, core, trail) = trimmed(runs);
        if core.is_empty() {
            self.range(runs, level + 1, after);
            return;
        }
        self.spaces(lead);
        let core_after = if trail > 0 { Some(' ') } else { after };
        match mark {
            Mark::Link => {
                if self.out.ends_with('!') {
                    self.out.pop();
                    self.out.push_str("\\!");
                }
                self.out.push('[');
                self.range(&core, level + 1, Some(']'));
                self.out.push_str("](<");
                destination(&mut self.out, key);
                self.out.push_str(">)");
            }
            Mark::Code => self.code(&core),
            Mark::Strike | Mark::Strong | Mark::Emphasis => {
                let delimiter = match mark {
                    Mark::Strike => "~~",
                    Mark::Strong => "**",
                    _ => "*",
                };
                if self.delimits(&core, level, core_after) {
                    self.out.push_str(delimiter);
                    self.range(&core, level + 1, delimiter.chars().next());
                    self.out.push_str(delimiter);
                } else {
                    self.range(&core, level + 1, core_after);
                }
            }
        }
        self.spaces(trail);
    }

    fn delimits(&self, core: &[Run<'_>], level: usize, after: Option<char>) -> bool {
        let (Some(first), Some(last)) = (core.first(), core.last()) else {
            return false;
        };
        let opening = match self.marked(first, level) {
            true => '*',
            false => first_char(core).unwrap_or(' '),
        };
        let closing = match self.marked(last, level) {
            true => '*',
            false => last.text.chars().next_back().unwrap_or(' '),
        };
        opens(self.out.chars().next_back(), opening) && closes(closing, after)
    }

    fn marked(&self, run: &Run<'_>, level: usize) -> bool {
        MARKS[level + 1..]
            .iter()
            .any(|&mark| self.key(mark, run.style).is_some())
    }

    fn code(&mut self, core: &[Run<'_>]) {
        let content: String = core.iter().map(|run| run.text).collect();
        let fence = "`".repeat(longest_backtick_run(&content) + 1);
        let padded = content.starts_with('`') || content.ends_with('`');
        self.out.push_str(&fence);
        if padded {
            self.out.push(' ');
        }
        self.out.push_str(&content);
        if padded {
            self.out.push(' ');
        }
        self.out.push_str(&fence);
    }

    fn escape(&mut self, text: &str) {
        let marked = match self.out.is_empty() {
            true => line_start_mark(text),
            false => None,
        };
        for (index, character) in text.char_indices() {
            if ESCAPED.contains(&character) || marked == Some(index) {
                self.out.push('\\');
            }
            self.out.push(character);
        }
    }

    fn spaces(&mut self, count: usize) {
        self.out.extend(std::iter::repeat_n(' ', count));
    }
}

fn trimmed<'a>(runs: &[Run<'a>]) -> (usize, Vec<Run<'a>>, usize) {
    let mut lead = 0;
    let mut trail = 0;
    let mut core: Vec<Run<'a>> = Vec::with_capacity(runs.len());
    for run in runs {
        let text = match core.is_empty() {
            true => {
                let kept = run.text.trim_start_matches(' ');
                lead += run.text.len() - kept.len();
                kept
            }
            false => run.text,
        };
        if !text.is_empty() {
            core.push(Run {
                text,
                style: run.style,
            });
        }
    }
    while let Some(last) = core.last_mut() {
        let kept = last.text.trim_end_matches(' ');
        trail += last.text.len() - kept.len();
        if kept.is_empty() {
            core.pop();
        } else {
            last.text = kept;
            break;
        }
    }
    (lead, core, trail)
}

fn first_char(runs: &[Run<'_>]) -> Option<char> {
    runs.iter().find_map(|run| run.text.chars().next())
}

fn line_start_mark(text: &str) -> Option<usize> {
    let first = text.chars().next()?;
    if LINE_START_ESCAPED.contains(&first) {
        return Some(0);
    }
    let digits = text.bytes().take_while(u8::is_ascii_digit).count();
    (digits > 0 && matches!(text.as_bytes().get(digits), Some(b'.' | b')'))).then_some(digits)
}

fn destination(out: &mut String, url: &str) {
    for character in url.chars() {
        match character {
            '<' => out.push_str("%3C"),
            '>' => out.push_str("%3E"),
            '\\' => out.push_str("%5C"),
            ' ' => out.push_str("%20"),
            '|' => out.push_str("%7C"),
            _ if character.is_control() => {}
            _ => out.push(character),
        }
    }
}

fn opens(before: Option<char>, next: char) -> bool {
    !next.is_whitespace() && (!maybe_punctuation(next) || before.is_none_or(is_open_side))
}

fn closes(previous: char, after: Option<char>) -> bool {
    !previous.is_whitespace() && (!maybe_punctuation(previous) || after.is_none_or(is_open_side))
}

fn is_open_side(character: char) -> bool {
    character.is_whitespace() || surely_punctuation(character)
}

fn maybe_punctuation(character: char) -> bool {
    !character.is_alphanumeric() && !character.is_whitespace()
}

fn surely_punctuation(character: char) -> bool {
    character.is_ascii_punctuation()
        || (!character.is_alphanumeric()
            && matches!(
                u32::from(character),
                0xa1..=0xbf
                    | 0xd7
                    | 0xf7
                    | 0x2010..=0x2027
                    | 0x2030..=0x205e
                    | 0x3001..=0x3003
                    | 0x3008..=0x3011
                    | 0x3014..=0x301f
                    | 0x3030
                    | 0x303d
                    | 0x30fb
                    | 0xff01..=0xff0f
                    | 0xff1a..=0xff20
                    | 0xff3b..=0xff40
                    | 0xff5b..=0xff65
            ))
}
