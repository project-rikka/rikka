use crate::composition::SegmentStyle;

const DECORATE: i32 = 1;
const FOREGROUND: i32 = 2;
const BACKGROUND: i32 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Attribute {
    pub(crate) style: SegmentStyle,
    pub(crate) start: usize,
    pub(crate) length: usize,
}

impl Attribute {
    pub(crate) fn parse(attributes: &str) -> Vec<Attribute> {
        attributes.split(';').filter_map(Attribute::one).collect()
    }

    fn one(group: &str) -> Option<Attribute> {
        let mut fields = group.split(':');
        let kind = fields.next()?.trim().parse::<i32>().ok()?;
        let start = fields.next()?.trim().parse::<usize>().ok()?;
        let length = fields.next()?.trim().parse::<usize>().ok()?;
        fields.next()?;
        let style = match kind {
            DECORATE => SegmentStyle::Underline,
            FOREGROUND | BACKGROUND => SegmentStyle::Highlight,
            _ => return None,
        };
        (length > 0).then_some(Attribute {
            style,
            start,
            length,
        })
    }
}
