use crate::properties::properties;
use crate::tables::enums::Script;
use crate::tables::scripts::{
    ALL_SCRIPTS, SCRIPT_CODES, SCRIPT_EXTENSION_STARTS, SCRIPT_EXTENSIONS,
};
use crate::tables::shaping::{
    SCRIPT_EXTENSIONS_MASK, SCRIPT_EXTENSIONS_SHIFT, SHAPING, SHAPING_VALUES,
};

pub type OpenTypeTag = [u8; 4];

static DEFAULT_TAGS: [OpenTypeTag; SCRIPT_CODES.len()] = {
    let mut tags = [[0; 4]; SCRIPT_CODES.len()];
    let mut index = 0;
    while index < SCRIPT_CODES.len() {
        let code = SCRIPT_CODES[index].as_bytes();
        tags[index] = [code[0].to_ascii_lowercase(), code[1], code[2], code[3]];
        index += 1;
    }
    tags
};

impl Script {
    pub fn code(self) -> &'static str {
        SCRIPT_CODES[self as usize]
    }

    pub fn from_code(code: &str) -> Option<Self> {
        let &[first, second, third, fourth] = code.as_bytes() else {
            return None;
        };
        let canonical = [
            first.to_ascii_uppercase(),
            second.to_ascii_lowercase(),
            third.to_ascii_lowercase(),
            fourth.to_ascii_lowercase(),
        ];
        SCRIPT_CODES
            .binary_search_by(|candidate| candidate.as_bytes().cmp(&canonical))
            .ok()
            .map(|index| ALL_SCRIPTS[index])
    }

    pub fn opentype_tags(self) -> &'static [OpenTypeTag] {
        match self {
            Self::Common | Self::Inherited | Self::Unknown => &[],
            Self::Bengali => const { &[*b"bng3", *b"bng2", *b"beng"] },
            Self::Devanagari => const { &[*b"dev3", *b"dev2", *b"deva"] },
            Self::Gujarati => const { &[*b"gjr3", *b"gjr2", *b"gujr"] },
            Self::Gurmukhi => const { &[*b"gur3", *b"gur2", *b"guru"] },
            Self::Kannada => const { &[*b"knd3", *b"knd2", *b"knda"] },
            Self::Malayalam => const { &[*b"mlm3", *b"mlm2", *b"mlym"] },
            Self::Oriya => const { &[*b"ory3", *b"ory2", *b"orya"] },
            Self::Tamil => const { &[*b"tml3", *b"tml2", *b"taml"] },
            Self::Telugu => const { &[*b"tel3", *b"tel2", *b"telu"] },
            Self::Myanmar => const { &[*b"mym2", *b"mymr"] },
            Self::Hiragana | Self::Katakana => const { &[*b"kana"] },
            Self::Lao => const { &[*b"lao "] },
            Self::Yi => const { &[*b"yi  "] },
            Self::Nko => const { &[*b"nko "] },
            Self::Vai => const { &[*b"vai "] },
            _ => std::slice::from_ref(&DEFAULT_TAGS[self as usize]),
        }
    }
}

pub fn script_extensions(character: char) -> &'static [Script] {
    let properties = properties(character);
    if !properties.has_script_extensions() {
        return std::slice::from_ref(&ALL_SCRIPTS[properties.script() as usize]);
    }
    let word = SHAPING_VALUES[usize::from(SHAPING.get(character as u32))];
    let set = (word >> SCRIPT_EXTENSIONS_SHIFT) & SCRIPT_EXTENSIONS_MASK;
    let set = set as usize - 1;
    let start = usize::from(SCRIPT_EXTENSION_STARTS[set]);
    let end = usize::from(SCRIPT_EXTENSION_STARTS[set + 1]);
    &SCRIPT_EXTENSIONS[start..end]
}

pub fn has_script(character: char, script: Script) -> bool {
    script_extensions(character).contains(&script)
}
