use rikka_types::Color;

use crate::parse::values::Stream;

const NAMED: [(&str, u32); 148] = [
    ("aliceblue", 0xf0f8ff),
    ("antiquewhite", 0xfaebd7),
    ("aqua", 0x00ffff),
    ("aquamarine", 0x7fffd4),
    ("azure", 0xf0ffff),
    ("beige", 0xf5f5dc),
    ("bisque", 0xffe4c4),
    ("black", 0x000000),
    ("blanchedalmond", 0xffebcd),
    ("blue", 0x0000ff),
    ("blueviolet", 0x8a2be2),
    ("brown", 0xa52a2a),
    ("burlywood", 0xdeb887),
    ("cadetblue", 0x5f9ea0),
    ("chartreuse", 0x7fff00),
    ("chocolate", 0xd2691e),
    ("coral", 0xff7f50),
    ("cornflowerblue", 0x6495ed),
    ("cornsilk", 0xfff8dc),
    ("crimson", 0xdc143c),
    ("cyan", 0x00ffff),
    ("darkblue", 0x00008b),
    ("darkcyan", 0x008b8b),
    ("darkgoldenrod", 0xb8860b),
    ("darkgray", 0xa9a9a9),
    ("darkgreen", 0x006400),
    ("darkgrey", 0xa9a9a9),
    ("darkkhaki", 0xbdb76b),
    ("darkmagenta", 0x8b008b),
    ("darkolivegreen", 0x556b2f),
    ("darkorange", 0xff8c00),
    ("darkorchid", 0x9932cc),
    ("darkred", 0x8b0000),
    ("darksalmon", 0xe9967a),
    ("darkseagreen", 0x8fbc8f),
    ("darkslateblue", 0x483d8b),
    ("darkslategray", 0x2f4f4f),
    ("darkslategrey", 0x2f4f4f),
    ("darkturquoise", 0x00ced1),
    ("darkviolet", 0x9400d3),
    ("deeppink", 0xff1493),
    ("deepskyblue", 0x00bfff),
    ("dimgray", 0x696969),
    ("dimgrey", 0x696969),
    ("dodgerblue", 0x1e90ff),
    ("firebrick", 0xb22222),
    ("floralwhite", 0xfffaf0),
    ("forestgreen", 0x228b22),
    ("fuchsia", 0xff00ff),
    ("gainsboro", 0xdcdcdc),
    ("ghostwhite", 0xf8f8ff),
    ("gold", 0xffd700),
    ("goldenrod", 0xdaa520),
    ("gray", 0x808080),
    ("green", 0x008000),
    ("greenyellow", 0xadff2f),
    ("grey", 0x808080),
    ("honeydew", 0xf0fff0),
    ("hotpink", 0xff69b4),
    ("indianred", 0xcd5c5c),
    ("indigo", 0x4b0082),
    ("ivory", 0xfffff0),
    ("khaki", 0xf0e68c),
    ("lavender", 0xe6e6fa),
    ("lavenderblush", 0xfff0f5),
    ("lawngreen", 0x7cfc00),
    ("lemonchiffon", 0xfffacd),
    ("lightblue", 0xadd8e6),
    ("lightcoral", 0xf08080),
    ("lightcyan", 0xe0ffff),
    ("lightgoldenrodyellow", 0xfafad2),
    ("lightgray", 0xd3d3d3),
    ("lightgreen", 0x90ee90),
    ("lightgrey", 0xd3d3d3),
    ("lightpink", 0xffb6c1),
    ("lightsalmon", 0xffa07a),
    ("lightseagreen", 0x20b2aa),
    ("lightskyblue", 0x87cefa),
    ("lightslategray", 0x778899),
    ("lightslategrey", 0x778899),
    ("lightsteelblue", 0xb0c4de),
    ("lightyellow", 0xffffe0),
    ("lime", 0x00ff00),
    ("limegreen", 0x32cd32),
    ("linen", 0xfaf0e6),
    ("magenta", 0xff00ff),
    ("maroon", 0x800000),
    ("mediumaquamarine", 0x66cdaa),
    ("mediumblue", 0x0000cd),
    ("mediumorchid", 0xba55d3),
    ("mediumpurple", 0x9370db),
    ("mediumseagreen", 0x3cb371),
    ("mediumslateblue", 0x7b68ee),
    ("mediumspringgreen", 0x00fa9a),
    ("mediumturquoise", 0x48d1cc),
    ("mediumvioletred", 0xc71585),
    ("midnightblue", 0x191970),
    ("mintcream", 0xf5fffa),
    ("mistyrose", 0xffe4e1),
    ("moccasin", 0xffe4b5),
    ("navajowhite", 0xffdead),
    ("navy", 0x000080),
    ("oldlace", 0xfdf5e6),
    ("olive", 0x808000),
    ("olivedrab", 0x6b8e23),
    ("orange", 0xffa500),
    ("orangered", 0xff4500),
    ("orchid", 0xda70d6),
    ("palegoldenrod", 0xeee8aa),
    ("palegreen", 0x98fb98),
    ("paleturquoise", 0xafeeee),
    ("palevioletred", 0xdb7093),
    ("papayawhip", 0xffefd5),
    ("peachpuff", 0xffdab9),
    ("peru", 0xcd853f),
    ("pink", 0xffc0cb),
    ("plum", 0xdda0dd),
    ("powderblue", 0xb0e0e6),
    ("purple", 0x800080),
    ("rebeccapurple", 0x663399),
    ("red", 0xff0000),
    ("rosybrown", 0xbc8f8f),
    ("royalblue", 0x4169e1),
    ("saddlebrown", 0x8b4513),
    ("salmon", 0xfa8072),
    ("sandybrown", 0xf4a460),
    ("seagreen", 0x2e8b57),
    ("seashell", 0xfff5ee),
    ("sienna", 0xa0522d),
    ("silver", 0xc0c0c0),
    ("skyblue", 0x87ceeb),
    ("slateblue", 0x6a5acd),
    ("slategray", 0x708090),
    ("slategrey", 0x708090),
    ("snow", 0xfffafa),
    ("springgreen", 0x00ff7f),
    ("steelblue", 0x4682b4),
    ("tan", 0xd2b48c),
    ("teal", 0x008080),
    ("thistle", 0xd8bfd8),
    ("tomato", 0xff6347),
    ("turquoise", 0x40e0d0),
    ("violet", 0xee82ee),
    ("wheat", 0xf5deb3),
    ("white", 0xffffff),
    ("whitesmoke", 0xf5f5f5),
    ("yellow", 0xffff00),
    ("yellowgreen", 0x9acd32),
];

pub(crate) fn color(text: &str) -> Option<Color> {
    let text = text.trim();
    if let Some(hex) = text.strip_prefix('#') {
        return hex_color(hex);
    }
    let lower = text.to_ascii_lowercase();
    if lower == "transparent" {
        return Some(Color::rgba(0.0, 0.0, 0.0, 0.0));
    }
    if let Some(arguments) = function(&lower, "rgba").or_else(|| function(&lower, "rgb")) {
        return rgb(arguments);
    }
    if let Some(arguments) = function(&lower, "hsla").or_else(|| function(&lower, "hsl")) {
        return hsl(arguments);
    }
    NAMED
        .binary_search_by(|(name, _)| (*name).cmp(lower.as_str()))
        .ok()
        .map(|at| Color::hex(NAMED[at].1))
}

fn function<'a>(text: &'a str, name: &str) -> Option<&'a str> {
    text.strip_prefix(name)?
        .trim_start()
        .strip_prefix('(')?
        .trim_end()
        .strip_suffix(')')
}

fn hex_color(hex: &str) -> Option<Color> {
    let digit = |at: usize| {
        hex.as_bytes()
            .get(at)
            .and_then(|byte| (*byte as char).to_digit(16))
    };
    let single = |at: usize| digit(at).map(|value| (value * 17) as f32 / 255.0);
    let double = |at: usize| Some((digit(at)? * 16 + digit(at + 1)?) as f32 / 255.0);
    if !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    match hex.len() {
        3 => Some(Color::rgba(single(0)?, single(1)?, single(2)?, 1.0)),
        4 => Some(Color::rgba(single(0)?, single(1)?, single(2)?, single(3)?)),
        6 => Some(Color::rgba(double(0)?, double(2)?, double(4)?, 1.0)),
        8 => Some(Color::rgba(double(0)?, double(2)?, double(4)?, double(6)?)),
        _ => None,
    }
}

fn components(arguments: &str) -> Option<Vec<(f32, bool)>> {
    let mut stream = Stream::new(arguments);
    let mut values = Vec::with_capacity(4);
    loop {
        stream.skip_separators();
        if stream.skip_if('/') {
            stream.skip_separators();
        }
        if stream.is_empty() {
            break;
        }
        let value = stream.number()?;
        let percent = stream.skip_if('%');
        if !percent {
            stream.skip_word();
        }
        values.push((value, percent));
    }
    matches!(values.len(), 3 | 4).then_some(values)
}

fn alpha(values: &[(f32, bool)]) -> f32 {
    match values.get(3) {
        Some(&(value, true)) => (value / 100.0).clamp(0.0, 1.0),
        Some(&(value, false)) => value.clamp(0.0, 1.0),
        None => 1.0,
    }
}

fn rgb(arguments: &str) -> Option<Color> {
    let values = components(arguments)?;
    let channel = |(value, percent): (f32, bool)| match percent {
        true => (value / 100.0).clamp(0.0, 1.0),
        false => (value / 255.0).clamp(0.0, 1.0),
    };
    Some(Color::rgba(
        channel(values[0]),
        channel(values[1]),
        channel(values[2]),
        alpha(&values),
    ))
}

fn hsl(arguments: &str) -> Option<Color> {
    let values = components(arguments)?;
    let hue = values[0].0.rem_euclid(360.0) / 60.0;
    let saturation = (values[1].0 / 100.0).clamp(0.0, 1.0);
    let lightness = (values[2].0 / 100.0).clamp(0.0, 1.0);
    let chroma = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation;
    let second = chroma * (1.0 - (hue % 2.0 - 1.0).abs());
    let (red, green, blue) = match hue as u32 {
        0 => (chroma, second, 0.0),
        1 => (second, chroma, 0.0),
        2 => (0.0, chroma, second),
        3 => (0.0, second, chroma),
        4 => (second, 0.0, chroma),
        _ => (chroma, 0.0, second),
    };
    let lift = lightness - chroma / 2.0;
    Some(Color::rgba(
        red + lift,
        green + lift,
        blue + lift,
        alpha(&values),
    ))
}
