use std::ops::RangeInclusive;

#[derive(Clone, Copy, Debug)]
pub struct Profile {
    pub name: &'static str,
    pub fonts: &'static [&'static str],
    pub ranges: &'static [RangeInclusive<u32>],
}

const JOINERS: RangeInclusive<u32> = 0x200C..=0x200D;
const SPACE: RangeInclusive<u32> = 0x20..=0x20;

pub const PROFILES: [Profile; 13] = [
    Profile {
        name: "phantom-advances",
        fonts: &[
            "InterVariable.ttf~HVAR",
            "NotoSans.ttf~HVAR",
            "NotoSansDevanagari.ttf~HVAR",
        ],
        ranges: &[
            0x41..=0x5A,
            0x61..=0x7A,
            0x300..=0x36F,
            0x400..=0x44F,
            0x915..=0x939,
            0x93E..=0x94D,
            SPACE,
        ],
    },
    Profile {
        name: "latin",
        fonts: &[
            "InterVariable.ttf",
            "NotoSans.ttf",
            "NotoSerif.ttf",
            "DejaVuSans.ttf",
        ],
        ranges: &[
            0x41..=0x5A,
            0x61..=0x7A,
            0x61..=0x7A,
            0x30..=0x39,
            0x21..=0x2F,
            SPACE,
            SPACE,
            0xA0..=0xFF,
            0x300..=0x36F,
            0x1E00..=0x1EFF,
            0x2000..=0x200F,
            0x2044..=0x2044,
            0xFE00..=0xFE0F,
            0x034F..=0x034F,
            0xAD..=0xAD,
            0xFB00..=0xFB06,
        ],
    },
    Profile {
        name: "greek-cyrillic",
        fonts: &["NotoSans.ttf", "NotoSans-Italic.ttf", "NotoSerif.ttf"],
        ranges: &[
            0x370..=0x3FF,
            0x1F00..=0x1FFF,
            0x400..=0x4FF,
            0x300..=0x36F,
            SPACE,
        ],
    },
    Profile {
        name: "arabic",
        fonts: &[
            "NotoSansArabic.ttf",
            "NotoNaskhArabic.ttf",
            "NotoKufiArabic.ttf",
            "NotoNastaliqUrdu.ttf",
        ],
        ranges: &[
            0x621..=0x64A,
            0x621..=0x64A,
            0x64B..=0x65F,
            0x670..=0x6D3,
            0x6D6..=0x6ED,
            0x6F0..=0x6F9,
            0x640..=0x640,
            0x8A0..=0x8FF,
            JOINERS,
            SPACE,
            0x660..=0x669,
            0xFE70..=0xFEFC,
        ],
    },
    Profile {
        name: "hebrew",
        fonts: &["NotoSansHebrew.ttf", "NotoSerifHebrew.ttf"],
        ranges: &[
            0x5D0..=0x5EA,
            0x5D0..=0x5EA,
            0x591..=0x5C7,
            0xFB1D..=0xFB4F,
            SPACE,
        ],
    },
    Profile {
        name: "thai-lao",
        fonts: &[
            "NotoSansThai.ttf",
            "NotoSerifThai.ttf",
            "NotoSansLao.ttf",
            "NotoSerifLao.ttf",
        ],
        ranges: &[0xE01..=0xE3A, 0xE40..=0xE4E, 0xE81..=0xEDF, SPACE],
    },
    Profile {
        name: "hangul",
        fonts: &["NotoSansCJK-VF.otf.ttc"],
        ranges: &[
            0xAC00..=0xD7A3,
            0x1100..=0x11FF,
            0x1100..=0x1112,
            0x1161..=0x1175,
            0x11A8..=0x11C2,
            0x302E..=0x302F,
            0xA960..=0xA97C,
            0xD7B0..=0xD7FB,
            SPACE,
        ],
    },
    Profile {
        name: "syriac",
        fonts: &["NotoSansSyriac.ttf", "NotoSansSyriacWestern.ttf"],
        ranges: &[0x710..=0x74A, 0x710..=0x72F, JOINERS, SPACE],
    },
    Profile {
        name: "nko",
        fonts: &["NotoSansNKo-Regular.otf"],
        ranges: &[0x7C0..=0x7FF, 0x7CA..=0x7EA, SPACE],
    },
    Profile {
        name: "mongolian",
        fonts: &["NotoSansMongolian-Regular.otf"],
        ranges: &[
            0x1820..=0x1878,
            0x1820..=0x1878,
            0x180B..=0x180F,
            0x1800..=0x180A,
            JOINERS,
            SPACE,
        ],
    },
    Profile {
        name: "cjk",
        fonts: &["NotoSansCJK-VF.otf.ttc"],
        ranges: &[
            0x4E00..=0x4FFF,
            0x3040..=0x30FF,
            0x3000..=0x303F,
            0xFE00..=0xFE0F,
            0xE0100..=0xE0110,
        ],
    },
    Profile {
        name: "emoji",
        fonts: &["NotoColorEmoji.ttf"],
        ranges: &[
            0x1F600..=0x1F64F,
            0x1F466..=0x1F469,
            0x1F3FB..=0x1F3FF,
            0x200D..=0x200D,
            0xFE0E..=0xFE0F,
            0x1F1E6..=0x1F1FF,
            0x20E3..=0x20E3,
            0x30..=0x39,
            0xE0061..=0xE007A,
            0xE007F..=0xE007F,
            0x2764..=0x2764,
        ],
    },
    Profile {
        name: "vietnamese",
        fonts: &["NotoSans.ttf", "InterVariable.ttf"],
        ranges: &[
            0x61..=0x7A,
            0x300..=0x309,
            0x323..=0x323,
            0x31B..=0x31B,
            0x1EA0..=0x1EF9,
            SPACE,
        ],
    },
];

#[derive(Clone, Debug)]
pub struct TextGenerator {
    state: u64,
}

impl TextGenerator {
    pub fn new(seed: u64) -> Self {
        Self {
            state: seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1,
        }
    }

    fn next(&mut self) -> u64 {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 7;
        self.state ^= self.state << 17;
        self.state
    }

    fn below(&mut self, bound: u64) -> u64 {
        self.next() % bound.max(1)
    }

    pub fn text(&mut self, profile: &Profile, max_len: u64) -> String {
        let len = 1 + self.below(max_len);
        let mut text = String::new();
        for _ in 0..len {
            let range = &profile.ranges[self.below(profile.ranges.len() as u64) as usize];
            let span = u64::from(range.end() - range.start()) + 1;
            let codepoint = range.start() + self.below(span) as u32;
            if let Some(character) = char::from_u32(codepoint) {
                text.push(character);
            }
        }
        text
    }
}

pub fn profile(name: &str) -> Option<&'static Profile> {
    PROFILES
        .iter()
        .chain(crate::syllabic_profiles::SYLLABIC_PROFILES.iter())
        .find(|profile| profile.name == name)
}
