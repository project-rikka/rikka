use std::borrow::Cow;

use crate::error::IesError;
use crate::profile::{Edition, IesProfile, Symmetry};

const TYPE_C: u32 = 1;

fn decode(bytes: &[u8]) -> Cow<'_, str> {
    match std::str::from_utf8(bytes) {
        Ok(text) => Cow::Borrowed(text),
        Err(_) => Cow::Owned(bytes.iter().map(|&byte| char::from(byte)).collect()),
    }
}

fn edition_of(header: &str) -> Option<Edition> {
    match header {
        "IESNA91" => Some(Edition::Lm1991),
        "IESNA:LM-63-1995" => Some(Edition::Lm1995),
        "IESNA:LM-63-2002" => Some(Edition::Lm2002),
        "IES:LM-63-2019" => Some(Edition::Lm2019),
        _ => None,
    }
}

struct Numbers<'a, I: Iterator<Item = &'a str>> {
    tokens: I,
}

impl<'a, I: Iterator<Item = &'a str>> Numbers<'a, I> {
    fn real(&mut self, what: &'static str) -> Result<f32, IesError> {
        let token = self.tokens.next().ok_or(IesError::Truncated(what))?;
        match token.parse::<f32>() {
            Ok(value) if value.is_finite() => Ok(value),
            _ => Err(IesError::Number {
                what,
                token: token.to_owned(),
            }),
        }
    }

    fn whole(&mut self, what: &'static str) -> Result<u32, IesError> {
        let value = self.real(what)?;
        match value >= 0.0 && value.fract() == 0.0 && value <= u32::MAX as f32 {
            true => Ok(value as u32),
            false => Err(IesError::Number {
                what,
                token: value.to_string(),
            }),
        }
    }

    fn angles(&mut self, what: &'static str, count: u32) -> Result<Vec<f32>, IesError> {
        let angles = (0..count)
            .map(|_| self.real(what))
            .collect::<Result<Vec<_>, _>>()?;
        if angles.windows(2).any(|pair| pair[1] <= pair[0]) {
            return Err(IesError::Angles {
                what,
                reason: "not ascending",
            });
        }
        Ok(angles)
    }
}

fn symmetry_of(horizontal: &[f32]) -> Result<Symmetry, IesError> {
    let (first, last) = (horizontal[0], horizontal[horizontal.len() - 1]);
    match (horizontal.len(), first, last) {
        (1, _, _) => Ok(Symmetry::Rotational),
        (_, 0.0, 90.0) => Ok(Symmetry::Quadrant),
        (_, 0.0, 180.0) => Ok(Symmetry::Bilateral),
        (_, 0.0, 360.0) => Ok(Symmetry::Full),
        (_, 90.0, 270.0) => Ok(Symmetry::Transverse),
        _ => Err(IesError::Angles {
            what: "horizontal",
            reason: "type C runs 0–90, 0–180, 0–360 or 90–270",
        }),
    }
}

pub(crate) fn parse(bytes: &[u8]) -> Result<IesProfile, IesError> {
    let text = decode(bytes);
    let mut lines = text.lines().map(str::trim);
    let mut edition = Edition::Lm1986;
    let mut keywords: Vec<(String, String)> = Vec::new();
    let mut tilt = None;
    for (number, line) in lines.by_ref().enumerate() {
        if number == 0
            && let Some(declared) = edition_of(line)
        {
            edition = declared;
            continue;
        }
        if let Some(value) = line.strip_prefix("TILT=") {
            tilt = Some(value.trim());
            break;
        }
        let Some((key, value)) = line.strip_prefix('[').and_then(|rest| rest.split_once(']'))
        else {
            continue;
        };
        let value = value.trim();
        match (key, keywords.last_mut()) {
            ("MORE", Some((_, previous))) => {
                previous.push('\n');
                previous.push_str(value);
            }
            _ => keywords.push((key.to_owned(), value.to_owned())),
        }
    }
    let tilt = tilt.ok_or(IesError::MissingTilt)?;
    if !tilt.eq_ignore_ascii_case("NONE") {
        return Err(IesError::Tilt(tilt.to_owned()));
    }

    let mut numbers = Numbers {
        tokens: lines
            .flat_map(|line| line.split(|c: char| c.is_whitespace() || c == ','))
            .filter(|token| !token.is_empty()),
    };
    let lamps = numbers.whole("lamp count")?;
    let lumens = numbers.real("lumens per lamp")?;
    let multiplier = numbers.real("candela multiplier")?;
    let vertical_count = numbers.whole("vertical angle count")?;
    let horizontal_count = numbers.whole("horizontal angle count")?;
    let photometric_type = numbers.whole("photometric type")?;
    numbers.whole("units type")?;
    for what in ["width", "length", "height"] {
        numbers.real(what)?;
    }
    let ballast = numbers.real("ballast factor")?;
    let lamp_factor = numbers.real("ballast-lamp photometric factor")?;
    let input_watts = numbers.real("input watts")?;
    if photometric_type != TYPE_C {
        return Err(IesError::PhotometricType(photometric_type));
    }
    if vertical_count < 2 || horizontal_count < 1 {
        return Err(IesError::Angles {
            what: "angle count",
            reason: "needs two vertical and one horizontal angle",
        });
    }
    let vertical = numbers.angles("vertical", vertical_count)?;
    if vertical[0] < 0.0 || vertical[vertical.len() - 1] > 180.0 {
        return Err(IesError::Angles {
            what: "vertical",
            reason: "type C lies within 0–180",
        });
    }
    let horizontal = numbers.angles("horizontal", horizontal_count)?;
    let symmetry = symmetry_of(&horizontal)?;
    let scale = multiplier
        * ballast
        * match edition < Edition::Lm2002 {
            true => lamp_factor,
            false => 1.0,
        };
    let candela = (0..vertical.len() * horizontal.len())
        .map(|_| numbers.real("candela values").map(|value| value * scale))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(IesProfile {
        edition,
        keywords,
        lamps,
        lumens_per_lamp: (lumens > 0.0).then_some(lumens),
        input_watts,
        vertical,
        horizontal,
        candela,
        symmetry,
    })
}
