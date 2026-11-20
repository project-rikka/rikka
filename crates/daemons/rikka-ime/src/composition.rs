use crate::attribute::Attribute;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SegmentStyle {
    #[default]
    Underline,
    Highlight,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Segment {
    pub text: String,
    pub style: SegmentStyle,
}

impl Segment {
    #[must_use]
    pub fn new(text: impl Into<String>, style: SegmentStyle) -> Self {
        Self {
            text: text.into(),
            style,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Preedit {
    segments: Vec<Segment>,
    cursor: usize,
}

impl Preedit {
    #[must_use]
    pub fn marked(text: &str, attributes: &str, caret: usize) -> Preedit {
        let characters: Vec<char> = text.chars().collect();
        let mut styles = vec![SegmentStyle::Underline; characters.len()];
        for attribute in Attribute::parse(attributes) {
            let end = attribute.start.saturating_add(attribute.length);
            for style in styles
                .iter_mut()
                .take(end)
                .skip(attribute.start.min(characters.len()))
            {
                *style = attribute.style;
            }
        }

        let mut segments: Vec<Segment> = Vec::new();
        for (character, style) in characters.iter().zip(styles) {
            match segments.last_mut() {
                Some(last) if last.style == style => last.text.push(*character),
                _ => segments.push(Segment::new(character.to_string(), style)),
            }
        }

        Preedit {
            cursor: at_character(&characters, caret),
            segments,
        }
    }

    #[must_use]
    pub fn segments(&self) -> &[Segment] {
        &self.segments
    }

    #[must_use]
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    #[must_use]
    pub fn text(&self) -> String {
        self.segments
            .iter()
            .map(|segment| segment.text.as_str())
            .collect()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }
}

fn at_character(text: &[char], characters: usize) -> usize {
    text.iter()
        .take(characters)
        .map(|character| character.len_utf8())
        .sum()
}
