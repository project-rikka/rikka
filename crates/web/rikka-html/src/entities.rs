use std::borrow::Cow;

const LONGEST_NAME: usize = 8;

const LATIN_START: u32 = 160;
const LATIN: [&str; 96] = [
    "nbsp", "iexcl", "cent", "pound", "curren", "yen", "brvbar", "sect", "uml", "copy", "ordf",
    "laquo", "not", "shy", "reg", "macr", "deg", "plusmn", "sup2", "sup3", "acute", "micro",
    "para", "middot", "cedil", "sup1", "ordm", "raquo", "frac14", "frac12", "frac34", "iquest",
    "Agrave", "Aacute", "Acirc", "Atilde", "Auml", "Aring", "AElig", "Ccedil", "Egrave", "Eacute",
    "Ecirc", "Euml", "Igrave", "Iacute", "Icirc", "Iuml", "ETH", "Ntilde", "Ograve", "Oacute",
    "Ocirc", "Otilde", "Ouml", "times", "Oslash", "Ugrave", "Uacute", "Ucirc", "Uuml", "Yacute",
    "THORN", "szlig", "agrave", "aacute", "acirc", "atilde", "auml", "aring", "aelig", "ccedil",
    "egrave", "eacute", "ecirc", "euml", "igrave", "iacute", "icirc", "iuml", "eth", "ntilde",
    "ograve", "oacute", "ocirc", "otilde", "ouml", "divide", "oslash", "ugrave", "uacute", "ucirc",
    "uuml", "yacute", "thorn", "yuml",
];

const GREEK_UPPER_START: u32 = 913;
const GREEK_UPPER: [&str; 25] = [
    "Alpha", "Beta", "Gamma", "Delta", "Epsilon", "Zeta", "Eta", "Theta", "Iota", "Kappa",
    "Lambda", "Mu", "Nu", "Xi", "Omicron", "Pi", "Rho", "", "Sigma", "Tau", "Upsilon", "Phi",
    "Chi", "Psi", "Omega",
];

const GREEK_LOWER_START: u32 = 945;
const GREEK_LOWER: [&str; 25] = [
    "alpha", "beta", "gamma", "delta", "epsilon", "zeta", "eta", "theta", "iota", "kappa",
    "lambda", "mu", "nu", "xi", "omicron", "pi", "rho", "sigmaf", "sigma", "tau", "upsilon", "phi",
    "chi", "psi", "omega",
];

const NAMED: [(&str, u32); 108] = [
    ("quot", 34),
    ("amp", 38),
    ("apos", 39),
    ("lt", 60),
    ("gt", 62),
    ("OElig", 338),
    ("oelig", 339),
    ("Scaron", 352),
    ("scaron", 353),
    ("Yuml", 376),
    ("fnof", 402),
    ("circ", 710),
    ("tilde", 732),
    ("thetasym", 977),
    ("upsih", 978),
    ("piv", 982),
    ("ensp", 8194),
    ("emsp", 8195),
    ("thinsp", 8201),
    ("zwnj", 8204),
    ("zwj", 8205),
    ("lrm", 8206),
    ("rlm", 8207),
    ("ndash", 8211),
    ("mdash", 8212),
    ("lsquo", 8216),
    ("rsquo", 8217),
    ("sbquo", 8218),
    ("ldquo", 8220),
    ("rdquo", 8221),
    ("bdquo", 8222),
    ("dagger", 8224),
    ("Dagger", 8225),
    ("bull", 8226),
    ("hellip", 8230),
    ("permil", 8240),
    ("prime", 8242),
    ("Prime", 8243),
    ("lsaquo", 8249),
    ("rsaquo", 8250),
    ("oline", 8254),
    ("frasl", 8260),
    ("euro", 8364),
    ("image", 8465),
    ("weierp", 8472),
    ("real", 8476),
    ("trade", 8482),
    ("alefsym", 8501),
    ("larr", 8592),
    ("uarr", 8593),
    ("rarr", 8594),
    ("darr", 8595),
    ("harr", 8596),
    ("crarr", 8629),
    ("lArr", 8656),
    ("uArr", 8657),
    ("rArr", 8658),
    ("dArr", 8659),
    ("hArr", 8660),
    ("forall", 8704),
    ("part", 8706),
    ("exist", 8707),
    ("empty", 8709),
    ("nabla", 8711),
    ("isin", 8712),
    ("notin", 8713),
    ("ni", 8715),
    ("prod", 8719),
    ("sum", 8721),
    ("minus", 8722),
    ("lowast", 8727),
    ("radic", 8730),
    ("prop", 8733),
    ("infin", 8734),
    ("ang", 8736),
    ("and", 8743),
    ("or", 8744),
    ("cap", 8745),
    ("cup", 8746),
    ("int", 8747),
    ("there4", 8756),
    ("sim", 8764),
    ("cong", 8773),
    ("asymp", 8776),
    ("ne", 8800),
    ("equiv", 8801),
    ("le", 8804),
    ("ge", 8805),
    ("sub", 8834),
    ("sup", 8835),
    ("nsub", 8836),
    ("sube", 8838),
    ("supe", 8839),
    ("oplus", 8853),
    ("otimes", 8855),
    ("perp", 8869),
    ("sdot", 8901),
    ("lceil", 8968),
    ("rceil", 8969),
    ("lfloor", 8970),
    ("rfloor", 8971),
    ("lang", 9001),
    ("rang", 9002),
    ("loz", 9674),
    ("spades", 9824),
    ("clubs", 9827),
    ("hearts", 9829),
    ("diams", 9830),
];

const WINDOWS_1252_START: u32 = 0x80;
const WINDOWS_1252: [u32; 32] = [
    0x20AC, 0x81, 0x201A, 0x0192, 0x201E, 0x2026, 0x2020, 0x2021, 0x02C6, 0x2030, 0x0160, 0x2039,
    0x0152, 0x8D, 0x017D, 0x8F, 0x90, 0x2018, 0x2019, 0x201C, 0x201D, 0x2022, 0x2013, 0x2014,
    0x02DC, 0x2122, 0x0161, 0x203A, 0x0153, 0x9D, 0x017E, 0x0178,
];

#[must_use]
pub fn decode(text: &str) -> Cow<'_, str> {
    let Some(first) = text.find('&') else {
        return Cow::Borrowed(text);
    };
    let mut decoded = String::with_capacity(text.len());
    decoded.push_str(&text[..first]);
    let mut rest = &text[first..];
    while let Some(at) = rest.find('&') {
        decoded.push_str(&rest[..at]);
        rest = &rest[at + 1..];
        match reference(rest) {
            Some((character, length)) => {
                decoded.push(character);
                rest = &rest[length..];
            }
            None => decoded.push('&'),
        }
    }
    decoded.push_str(rest);
    Cow::Owned(decoded)
}

fn reference(text: &str) -> Option<(char, usize)> {
    match text.strip_prefix('#') {
        Some(number) => numeric(number).map(|(character, length)| (character, length + 1)),
        None => named(text),
    }
}

fn numeric(text: &str) -> Option<(char, usize)> {
    let (digits, radix, prefix) = match text.strip_prefix(['x', 'X']) {
        Some(hex) => (hex, 16, 1),
        None => (text, 10, 0),
    };
    let length = digits
        .bytes()
        .take_while(|byte| char::from(*byte).is_digit(radix))
        .count();
    if length == 0 {
        return None;
    }
    let code = u32::from_str_radix(&digits[..length], radix).unwrap_or(u32::MAX);
    let terminator = usize::from(digits[length..].starts_with(';'));
    Some((scalar(code), prefix + length + terminator))
}

fn scalar(code: u32) -> char {
    let code = match code.checked_sub(WINDOWS_1252_START) {
        Some(offset) => WINDOWS_1252.get(offset as usize).copied().unwrap_or(code),
        None => code,
    };
    match code {
        0 => char::REPLACEMENT_CHARACTER,
        code => char::from_u32(code).unwrap_or(char::REPLACEMENT_CHARACTER),
    }
}

fn named(text: &str) -> Option<(char, usize)> {
    let length = text
        .bytes()
        .take(LONGEST_NAME + 1)
        .take_while(u8::is_ascii_alphanumeric)
        .count();
    if text.as_bytes().get(length) != Some(&b';') {
        return None;
    }
    let name = &text[..length];
    let code = run(&LATIN, LATIN_START, name)
        .or_else(|| run(&GREEK_UPPER, GREEK_UPPER_START, name))
        .or_else(|| run(&GREEK_LOWER, GREEK_LOWER_START, name))
        .or_else(|| {
            NAMED
                .iter()
                .find(|(known, _)| *known == name)
                .map(|(_, code)| *code)
        })?;
    Some((char::from_u32(code)?, length + 1))
}

fn run(names: &[&str], start: u32, name: &str) -> Option<u32> {
    let index = names
        .iter()
        .position(|known| !known.is_empty() && *known == name)?;
    Some(start + u32::try_from(index).ok()?)
}
