use rikka_vector::Transform;

#[derive(Clone, Debug)]
pub(crate) struct Stream<'a> {
    text: &'a str,
    at: usize,
}

impl<'a> Stream<'a> {
    pub(crate) fn new(text: &'a str) -> Stream<'a> {
        Stream { text, at: 0 }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.at >= self.text.len()
    }

    pub(crate) fn rest(&self) -> &'a str {
        &self.text[self.at..]
    }

    pub(crate) fn peek(&self) -> Option<u8> {
        self.text.as_bytes().get(self.at).copied()
    }

    pub(crate) fn skip_spaces(&mut self) {
        while self.peek().is_some_and(|byte| byte.is_ascii_whitespace()) {
            self.at += 1;
        }
    }

    pub(crate) fn skip_separators(&mut self) {
        self.skip_spaces();
        if self.peek() == Some(b',') {
            self.at += 1;
            self.skip_spaces();
        }
    }

    pub(crate) fn skip_if(&mut self, expected: char) -> bool {
        let found = self.rest().starts_with(expected);
        if found {
            self.at += expected.len_utf8();
        }
        found
    }

    pub(crate) fn skip_word(&mut self) {
        while self.peek().is_some_and(|byte| byte.is_ascii_alphabetic()) {
            self.at += 1;
        }
    }

    pub(crate) fn word(&mut self) -> &'a str {
        let start = self.at;
        while self
            .peek()
            .is_some_and(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
        {
            self.at += 1;
        }
        &self.text[start..self.at]
    }

    pub(crate) fn number(&mut self) -> Option<f32> {
        let bytes = self.text.as_bytes();
        let start = self.at;
        let mut at = start;
        if matches!(bytes.get(at), Some(b'+' | b'-')) {
            at += 1;
        }
        let digits_from = at;
        while bytes.get(at).is_some_and(u8::is_ascii_digit) {
            at += 1;
        }
        let mut digits = at - digits_from;
        if bytes.get(at) == Some(&b'.') {
            at += 1;
            let fraction_from = at;
            while bytes.get(at).is_some_and(u8::is_ascii_digit) {
                at += 1;
            }
            digits += at - fraction_from;
        }
        if digits == 0 {
            return None;
        }
        if matches!(bytes.get(at), Some(b'e' | b'E')) {
            let mut exponent = at + 1;
            if matches!(bytes.get(exponent), Some(b'+' | b'-')) {
                exponent += 1;
            }
            if bytes.get(exponent).is_some_and(u8::is_ascii_digit) {
                while bytes.get(exponent).is_some_and(u8::is_ascii_digit) {
                    exponent += 1;
                }
                at = exponent;
            }
        }
        let value: f32 = self.text[start..at].parse().ok()?;
        self.at = at;
        value.is_finite().then_some(value)
    }
}

pub(crate) fn number(text: &str) -> Option<f32> {
    let mut stream = Stream::new(text.trim());
    let value = stream.number()?;
    stream.is_empty().then_some(value)
}

pub(crate) fn numbers(text: &str) -> Vec<f32> {
    let mut stream = Stream::new(text);
    let mut values = Vec::new();
    loop {
        stream.skip_separators();
        match stream.number() {
            Some(value) => values.push(value),
            None => return values,
        }
    }
}

pub(crate) fn unit_interval(text: &str) -> Option<f32> {
    let mut stream = Stream::new(text.trim());
    let value = stream.number()?;
    let value = match stream.skip_if('%') {
        true => value / 100.0,
        false => value,
    };
    stream.is_empty().then_some(value.clamp(0.0, 1.0))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Unit {
    User,
    Em,
    Ex,
    Percent,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Length {
    pub(crate) value: f32,
    pub(crate) unit: Unit,
}

impl Length {
    pub(crate) const ZERO: Length = Length {
        value: 0.0,
        unit: Unit::User,
    };

    pub(crate) const fn percent(value: f32) -> Length {
        Length {
            value,
            unit: Unit::Percent,
        }
    }
}

fn length_in(stream: &mut Stream<'_>) -> Option<Length> {
    let value = stream.number()?;
    if stream.skip_if('%') {
        return Some(Length::percent(value));
    }
    let (scale, unit) = match stream.word().to_ascii_lowercase().as_str() {
        "" | "px" => (1.0, Unit::User),
        "pt" => (4.0 / 3.0, Unit::User),
        "pc" => (16.0, Unit::User),
        "mm" => (96.0 / 25.4, Unit::User),
        "cm" => (96.0 / 2.54, Unit::User),
        "in" => (96.0, Unit::User),
        "q" => (96.0 / 101.6, Unit::User),
        "em" | "rem" => (1.0, Unit::Em),
        "ex" => (1.0, Unit::Ex),
        _ => return None,
    };
    Some(Length {
        value: value * scale,
        unit,
    })
}

pub(crate) fn length(text: &str) -> Option<Length> {
    let mut stream = Stream::new(text.trim());
    let length = length_in(&mut stream)?;
    stream.is_empty().then_some(length)
}

pub(crate) fn lengths(text: &str) -> Option<Vec<Length>> {
    let mut stream = Stream::new(text.trim());
    let mut values = Vec::new();
    while !stream.is_empty() {
        values.push(length_in(&mut stream)?);
        stream.skip_separators();
    }
    Some(values)
}

pub(crate) fn transform(text: &str) -> Option<Transform> {
    let mut stream = Stream::new(text);
    let mut total = Transform::IDENTITY;
    loop {
        stream.skip_separators();
        if stream.is_empty() {
            return Some(total);
        }
        let name = stream.word();
        stream.skip_spaces();
        if !stream.skip_if('(') {
            return None;
        }
        let mut arguments = [0.0f32; 6];
        let mut count = 0;
        loop {
            stream.skip_separators();
            if stream.skip_if(')') {
                break;
            }
            let value = stream.number()?;
            stream.skip_word();
            if count == arguments.len() {
                return None;
            }
            arguments[count] = value;
            count += 1;
        }
        let [a, b, c, d, e, f] = arguments;
        let next = match (name, count) {
            ("matrix", 6) => Transform::new(a, b, c, d, e, f),
            ("translate", 1 | 2) => Transform::translate(a, b),
            ("scale", 1) => Transform::scale(a, a),
            ("scale", 2) => Transform::scale(a, b),
            ("rotate", 1) => Transform::rotate(a),
            ("rotate", 3) => Transform::rotate(a).around(rikka_types::Point::new(b, c)),
            ("skewX", 1) => Transform::skew(a, 0.0),
            ("skewY", 1) => Transform::skew(0.0, a),
            _ => return None,
        };
        total = next.then(total);
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ViewBox {
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) width: f32,
    pub(crate) height: f32,
}

pub(crate) fn view_box(text: &str) -> Option<ViewBox> {
    match numbers(text).as_slice() {
        &[x, y, width, height] if width > 0.0 && height > 0.0 => Some(ViewBox {
            x,
            y,
            width,
            height,
        }),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct AspectRatio {
    pub(crate) align: Option<(f32, f32)>,
    pub(crate) slice: bool,
}

impl Default for AspectRatio {
    fn default() -> Self {
        AspectRatio {
            align: Some((0.5, 0.5)),
            slice: false,
        }
    }
}

pub(crate) fn aspect_ratio(text: &str) -> AspectRatio {
    let mut words = text
        .split_ascii_whitespace()
        .filter(|word| *word != "defer");
    let align = match words.next() {
        Some("none") => None,
        Some(word) if word.len() == 8 => {
            let share = |part: &str| match part.to_ascii_lowercase().as_str() {
                "min" => Some(0.0),
                "mid" => Some(0.5),
                "max" => Some(1.0),
                _ => None,
            };
            match (share(&word[1..4]), share(&word[5..8])) {
                (Some(x), Some(y)) => Some((x, y)),
                _ => Some((0.5, 0.5)),
            }
        }
        _ => Some((0.5, 0.5)),
    };
    AspectRatio {
        align,
        slice: words.next() == Some("slice"),
    }
}

pub(crate) fn fit(view: ViewBox, aspect: AspectRatio, width: f32, height: f32) -> Transform {
    let (scale_x, scale_y) = (width / view.width, height / view.height);
    let Some((align_x, align_y)) = aspect.align else {
        return Transform::translate(-view.x, -view.y).then(Transform::scale(scale_x, scale_y));
    };
    let scale = match aspect.slice {
        true => scale_x.max(scale_y),
        false => scale_x.min(scale_y),
    };
    let left = (width - view.width * scale) * align_x;
    let top = (height - view.height * scale) * align_y;
    Transform::translate(-view.x, -view.y)
        .then(Transform::scale(scale, scale))
        .then(Transform::translate(left, top))
}

pub(crate) fn reference(text: &str) -> Option<&str> {
    let text = text.trim();
    let inner = text
        .strip_prefix("url(")
        .and_then(|rest| rest.split_once(')'))
        .map_or(text, |(inner, _)| inner);
    let inner = inner.trim().trim_matches(['"', '\'']);
    inner.strip_prefix('#').filter(|id| !id.is_empty())
}

pub(crate) fn after_reference(text: &str) -> Option<&str> {
    let (_, rest) = text.trim().strip_prefix("url(")?.split_once(')')?;
    let rest = rest.trim();
    (!rest.is_empty()).then_some(rest)
}
