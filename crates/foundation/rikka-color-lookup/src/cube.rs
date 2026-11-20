use crate::error::ParseError;
use crate::lookup::{ColorLookup, Domain};

pub(crate) fn is_cube(text: &str) -> bool {
    text.lines()
        .map(str::trim_start)
        .any(|line| line.starts_with("LUT_3D_SIZE") || line.starts_with("LUT_1D_SIZE"))
}

pub(crate) fn parse(text: &str) -> Result<ColorLookup, ParseError> {
    let mut size = None;
    let mut domain = Domain::default();
    let mut table = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let line = line.split('#').next().unwrap_or_default().trim();
        if line.is_empty() {
            continue;
        }
        let fail = |reason| ParseError::Line {
            line: index + 1,
            reason,
        };
        let mut parts = line.split_whitespace();
        let Some(first) = parts.next() else {
            continue;
        };
        if !first.starts_with(|start: char| start.is_ascii_alphabetic()) {
            let red = number(first).ok_or_else(|| fail("expected a number"))?;
            let rest = triple_tail(&mut parts).ok_or_else(|| fail("expected three numbers"))?;
            table.extend([red, rest[0], rest[1]]);
            continue;
        }
        match first {
            "LUT_3D_SIZE" => {
                let value = parts
                    .next()
                    .and_then(|value| value.parse().ok())
                    .ok_or_else(|| fail("LUT_3D_SIZE needs a whole number"))?;
                size = Some(value);
            }
            "LUT_1D_SIZE" => return Err(ParseError::OneDimensional),
            "DOMAIN_MIN" => {
                domain.min =
                    triple(&mut parts).ok_or_else(|| fail("DOMAIN_MIN needs three numbers"))?
            }
            "DOMAIN_MAX" => {
                domain.max =
                    triple(&mut parts).ok_or_else(|| fail("DOMAIN_MAX needs three numbers"))?
            }
            "LUT_3D_INPUT_RANGE" => {
                let low = parts.next().and_then(number);
                let high = parts.next().and_then(number);
                let (Some(low), Some(high)) = (low, high) else {
                    return Err(fail("LUT_3D_INPUT_RANGE needs two numbers"));
                };
                domain = Domain {
                    min: [low; 3],
                    max: [high; 3],
                };
            }
            _ => {}
        }
    }
    let size = size.ok_or(ParseError::Unrecognized)?;
    ColorLookup::new(size, domain, table)
}

fn number(text: &str) -> Option<f32> {
    text.parse::<f32>().ok().filter(|value| value.is_finite())
}

fn triple<'a>(parts: &mut impl Iterator<Item = &'a str>) -> Option<[f32; 3]> {
    let values = [parts.next()?, parts.next()?, parts.next()?];
    let values = [number(values[0])?, number(values[1])?, number(values[2])?];
    parts.next().is_none().then_some(values)
}

fn triple_tail<'a>(parts: &mut impl Iterator<Item = &'a str>) -> Option<[f32; 2]> {
    let values = [number(parts.next()?)?, number(parts.next()?)?];
    parts.next().is_none().then_some(values)
}
