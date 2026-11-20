use crate::error::Result;
use crate::line::ContentLine;

const BYTE_ORDER_MARK: char = '\u{feff}';

pub fn read_lines(text: &str) -> Result<Vec<ContentLine>> {
    let text = text.strip_prefix(BYTE_ORDER_MARK).unwrap_or(text);
    let mut lines = Vec::new();
    let mut pending: Option<(String, usize)> = None;

    for (index, physical) in text.split('\n').enumerate() {
        let physical = physical.strip_suffix('\r').unwrap_or(physical);
        if let Some(continued) = physical
            .strip_prefix(' ')
            .or_else(|| physical.strip_prefix('\t'))
            && let Some((pending_line, _)) = pending.as_mut()
        {
            pending_line.push_str(continued);
            continue;
        }
        if let Some((pending_line, number)) = pending.take() {
            lines.push(ContentLine::parse_numbered(&pending_line, number)?);
        }
        if !physical.trim().is_empty() {
            pending = Some((physical.to_owned(), index + 1));
        }
    }
    if let Some((pending_line, number)) = pending {
        lines.push(ContentLine::parse_numbered(&pending_line, number)?);
    }

    Ok(lines)
}
