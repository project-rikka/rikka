use crate::error::ParseError;
use crate::lookup::{ColorLookup, Domain, LARGEST};

pub(crate) fn parse(text: &str) -> Result<ColorLookup, ParseError> {
    let rows: Vec<(usize, Vec<f32>)> = text
        .lines()
        .enumerate()
        .map(|(index, line)| (index, line.split('#').next().unwrap_or_default().trim()))
        .filter(|(_, line)| !line.is_empty())
        .filter_map(|(index, line)| {
            line.split_whitespace()
                .map(|value| value.parse::<f32>().ok().filter(|value| value.is_finite()))
                .collect::<Option<Vec<f32>>>()
                .map(|values| (index, values))
        })
        .collect();
    let header = rows
        .iter()
        .position(|(_, row)| row.len() > 3)
        .ok_or(ParseError::Unrecognized)?;
    let size = rows[header].1.len();
    if !(2..=LARGEST).contains(&size) {
        return Err(ParseError::Size(size));
    }
    let max_input = rows[header].1.last().copied().unwrap_or(1.0).max(1.0);
    let mut entries = Vec::with_capacity(size * size * size);
    for (index, row) in &rows[header + 1..] {
        match row.len() {
            3 => entries.push([row[0], row[1], row[2]]),
            _ => {
                return Err(ParseError::Line {
                    line: index + 1,
                    reason: "expected three numbers",
                });
            }
        }
    }
    let expected = size * size * size;
    if entries.len() != expected {
        return Err(ParseError::Entries {
            size,
            expected,
            found: entries.len(),
        });
    }
    let peak = entries.iter().flatten().fold(1.0f32, |a, &b| a.max(b));
    let scale = match peak {
        peak if peak > 4095.0 => 65535.0,
        peak if peak > 1023.0 => 4095.0,
        peak => max_input.max(peak),
    };
    let mut table = vec![0.0f32; expected * 3];
    for red in 0..size {
        for green in 0..size {
            for blue in 0..size {
                let source = entries[(red * size + green) * size + blue];
                let at = ((blue * size + green) * size + red) * 3;
                table[at..at + 3].copy_from_slice(&source.map(|value| value / scale));
            }
        }
    }
    ColorLookup::new(size, Domain::default(), table)
}
