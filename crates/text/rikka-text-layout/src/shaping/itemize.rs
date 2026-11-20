use std::ops::Range;
use std::sync::OnceLock;

use rikka_text_catalog::{Catalog, Face, FaceMatch, FallbackRequest};
use rikka_text_unicode::{
    EmojiPresentation, GeneralCategory, Level, Properties, Script, emoji_presentation, graphemes,
    properties, script_runs,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Item {
    pub(crate) start: u32,
    pub(crate) end: u32,
    pub(crate) level: Level,
    pub(crate) script: Script,
    pub(crate) style: u16,
    pub(crate) face: Option<u16>,
}

pub(crate) struct Itemizer<'a> {
    pub(crate) catalog: &'a Catalog,
    pub(crate) text: &'a str,
    pub(crate) levels: &'a [Level],
    pub(crate) spans: &'a [(Range<usize>, u16)],
    pub(crate) primaries: &'a [Option<FaceMatch>],
}

struct Placement<'a> {
    items: &'a mut Vec<Item>,
    matches: &'a mut Vec<FaceMatch>,
    primaries: &'a mut Vec<Option<u16>>,
    ascii: Vec<u128>,
    span: usize,
}

impl Placement<'_> {
    fn intern(&mut self, face: FaceMatch) -> u16 {
        match self.matches.iter().position(|known| *known == face) {
            Some(index) => index as u16,
            None => {
                self.matches.push(face);
                (self.matches.len() - 1) as u16
            }
        }
    }

    fn previous_face(&self, style: u16) -> Option<u16> {
        self.items
            .last()
            .filter(|item| item.style == style)
            .and_then(|item| item.face)
    }

    fn push(&mut self, item: Item) {
        match self.items.last_mut() {
            Some(last)
                if last.end == item.start
                    && last.level == item.level
                    && last.script == item.script
                    && last.style == item.style
                    && last.face == item.face =>
            {
                last.end = item.end;
            }
            _ => self.items.push(item),
        }
    }
}

impl Itemizer<'_> {
    pub(crate) fn itemize(
        &self,
        items: &mut Vec<Item>,
        matches: &mut Vec<FaceMatch>,
        primaries: &mut Vec<Option<u16>>,
    ) {
        items.clear();
        matches.clear();
        primaries.clear();
        let mut placement = Placement {
            items,
            matches,
            primaries,
            ascii: Vec::with_capacity(self.primaries.len()),
            span: 0,
        };
        for primary in self.primaries {
            let interned = primary.map(|primary| placement.intern(primary));
            placement.primaries.push(interned);
            placement.ascii.push(primary.map_or(0, |primary| {
                ascii_coverage(self.catalog.face(primary.face()))
            }));
        }
        for run in script_runs(self.text) {
            let run_text = &self.text[run.range.clone()];
            if run_text
                .bytes()
                .all(|byte| byte.is_ascii() && byte != b'\r')
            {
                self.place_ascii(&mut placement, run.range, run.script);
            } else {
                for (offset, cluster) in graphemes(run_text) {
                    self.place(
                        &mut placement,
                        run.range.start + offset,
                        cluster,
                        run.script,
                    );
                }
            }
        }
    }

    fn place_ascii(&self, placement: &mut Placement<'_>, range: Range<usize>, script: Script) {
        let bytes = self.text.as_bytes();
        let mut start = range.start;
        while start < range.end {
            let style = self.style_at(placement, start);
            let primary = placement.primaries[usize::from(style)];
            let uses_primary = primary.is_some()
                && placement
                    .items
                    .last()
                    .is_none_or(|item| item.style != style || item.face == primary);
            let level = self.level_at(start);
            let mask = placement.ascii[usize::from(style)];
            let limit = self.style_end(placement, start).min(range.end);
            let end = if uses_primary {
                (start..limit)
                    .find(|&offset| {
                        mask & (1 << bytes[offset]) == 0
                            || self.levels.get(offset).is_some_and(|&other| other != level)
                    })
                    .unwrap_or(limit)
            } else {
                start
            };
            if end == start {
                self.place(placement, start, &self.text[start..start + 1], script);
                start += 1;
                continue;
            }
            placement.push(Item {
                start: start as u32,
                end: end as u32,
                level,
                script,
                style,
                face: primary,
            });
            start = end;
        }
    }

    fn place(&self, placement: &mut Placement<'_>, start: usize, cluster: &str, script: Script) {
        let style = self.style_at(placement, start);
        let face = self.face_for(placement, cluster, style, script);
        placement.push(Item {
            start: start as u32,
            end: (start + cluster.len()) as u32,
            level: self.level_at(start),
            script,
            style,
            face,
        });
    }

    fn style_at(&self, placement: &mut Placement<'_>, start: usize) -> u16 {
        while placement.span < self.spans.len() && self.spans[placement.span].0.end <= start {
            placement.span += 1;
        }
        match self.spans.get(placement.span) {
            Some((range, style)) if range.start <= start => *style,
            _ => 0,
        }
    }

    fn style_end(&self, placement: &Placement<'_>, start: usize) -> usize {
        match self.spans.get(placement.span) {
            Some((range, _)) if range.start <= start => range.end,
            Some((range, _)) => range.start,
            None => usize::MAX,
        }
    }

    fn level_at(&self, start: usize) -> Level {
        self.levels.get(start).copied().unwrap_or_default()
    }

    fn face_for(
        &self,
        placement: &mut Placement<'_>,
        cluster: &str,
        style: u16,
        script: Script,
    ) -> Option<u16> {
        let primary = placement.primaries[usize::from(style)]?;
        let previous = placement.previous_face(style);
        if let &[byte] = cluster.as_bytes()
            && previous.is_none_or(|previous| previous == primary)
            && placement.ascii[usize::from(style)] & (1 << byte) != 0
        {
            return Some(primary);
        }
        let mut characters = cluster.chars();
        let first = characters.next()?;
        let first_properties = properties(first);
        let single = characters.next().is_none();
        let prefers_color = if single {
            first_properties.is_emoji_presentation()
        } else {
            emoji_presentation(cluster) == Some(EmojiPresentation::Emoji)
        };
        let covers = |face: &Face| {
            if single {
                face.covers(first) || is_invisible(first_properties)
            } else {
                covers(face, cluster)
            }
        };
        let previous = previous
            .filter(|&previous| previous != primary)
            .map(|previous| {
                let face = placement.matches[usize::from(previous)].face();
                (previous, self.catalog.face(face))
            });
        let neutral = matches!(
            first_properties.script(),
            Script::Common | Script::Inherited
        );
        if let Some((previous, face)) = previous
            && neutral
            && !prefers_color
            && !face.is_color()
            && covers(face)
        {
            return Some(previous);
        }
        let primary_match = placement.matches[usize::from(primary)];
        if !prefers_color && covers(self.catalog.face(primary_match.face())) {
            return Some(primary);
        }
        if let Some((previous, face)) = previous
            && face.is_color() == prefers_color
            && covers(face)
        {
            return Some(previous);
        }
        let fallback = self
            .catalog
            .fallback(&FallbackRequest::new(primary_match, cluster).script(script));
        Some(placement.intern(fallback))
    }
}

fn ascii_coverage(face: &Face) -> u128 {
    static INVISIBLE: OnceLock<u128> = OnceLock::new();
    let invisible = INVISIBLE.get_or_init(|| {
        (0u8..128)
            .filter(|&byte| is_invisible(properties(char::from(byte))))
            .fold(0, |mask, byte| mask | 1 << byte)
    });
    face.ascii_coverage() | invisible
}

fn covers(face: &Face, cluster: &str) -> bool {
    cluster
        .chars()
        .all(|character| face.covers(character) || is_invisible(properties(character)))
}

fn is_invisible(properties: Properties) -> bool {
    properties.is_default_ignorable()
        || matches!(
            properties.general_category(),
            GeneralCategory::Control
                | GeneralCategory::LineSeparator
                | GeneralCategory::ParagraphSeparator
        )
}
