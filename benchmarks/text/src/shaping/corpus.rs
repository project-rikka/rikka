use crate::syllabic_corpus::syllabic_cases;

#[derive(Clone, Copy, Debug)]
pub struct Case {
    pub group: &'static str,
    pub font: &'static str,
    pub face: u32,
    pub text: &'static str,
    pub script: Option<&'static str>,
    pub rtl: Option<bool>,
    pub language: Option<&'static str>,
    pub features: &'static str,
    pub variations: &'static [(&'static [u8; 4], f32)],
    pub bot_eot: bool,
}

impl Case {
    pub const fn new(group: &'static str, font: &'static str, text: &'static str) -> Self {
        Self {
            group,
            font,
            face: 0,
            text,
            script: None,
            rtl: None,
            language: None,
            features: "",
            variations: &[],
            bot_eot: false,
        }
    }

    pub const fn face(mut self, face: u32) -> Self {
        self.face = face;
        self
    }

    pub const fn script(mut self, script: &'static str) -> Self {
        self.script = Some(script);
        self
    }

    pub const fn rtl(mut self, rtl: bool) -> Self {
        self.rtl = Some(rtl);
        self
    }

    pub const fn language(mut self, language: &'static str) -> Self {
        self.language = Some(language);
        self
    }

    pub const fn features(mut self, features: &'static str) -> Self {
        self.features = features;
        self
    }

    pub const fn variations(mut self, variations: &'static [(&'static [u8; 4], f32)]) -> Self {
        self.variations = variations;
        self
    }

    pub const fn bot_eot(mut self) -> Self {
        self.bot_eot = true;
        self
    }
}

const LATIN_TEXTS: [&str; 8] = [
    "The quick brown fox jumps over the lazy dog.",
    "office affluent fjord flag ffi ffl Th st",
    "AVATAR Tj To Ty Wa We Yo LT P. V, F. \"quoted\" 'single'",
    "1/2 3/4 2024-10-01 12:30 +1 (555) 010-9999 #hashtag @user",
    "e\u{301}te\u{301} a\u{308}\u{301} q\u{323}\u{302} i\u{307}\u{301} n\u{303} o\u{328}\u{304}",
    "Ångström Øresund Ærøskøbing Œuvre naïve façade über straße",
    "x\u{300}\u{301}\u{302}\u{303}\u{304}\u{305}\u{306}\u{307}\u{308}\u{309}\u{30A}",
    "soft\u{AD}hyphen zero\u{200B}width word\u{2060}joiner no\u{A0}break thin\u{2009}space en\u{2002}space",
];

const GREEK_TEXTS: [&str; 3] = [
    "Καλημέρα κόσμε, ἄλφα βῆτα γάμμα δέλτα ᾅδης ῥήτωρ",
    "α\u{301}\u{345} η\u{313}\u{342}\u{345} ω\u{314}\u{301} Ι\u{308}\u{301}",
    "ΑΥΤΟΚΙΝΗΤΟ Ἀθῆναι ΰ ΐ",
];

const CYRILLIC_TEXTS: [&str; 3] = [
    "Съешь же ещё этих мягких французских булок, да выпей чаю.",
    "й и\u{306} ё е\u{308} Ѓ Ќ ї ґ",
    "бгдпт абвгдежзийклмнопрстуфхцчшщъыьэюя",
];

const VIETNAMESE_TEXTS: [&str; 2] = [
    "Tiếng Việt có dấu: ẩ ễ ộ ự ẳ ờ ỡ ẫ ậ",
    "e\u{302}\u{301} o\u{31B}\u{323} a\u{306}\u{309} u\u{31B}\u{300}",
];

const ARABIC_TEXTS: [&str; 9] = [
    "السلام عليكم ورحمة الله وبركاته",
    "بِسْمِ ٱللَّٰهِ ٱلرَّحْمَٰنِ ٱلرَّحِيمِ",
    "لا إله إلا الله محمد رسول الله",
    "مرحبا بالعالم ١٢٣٤ 2025",
    "ب\u{200D} \u{200D}ب ب\u{200C}ب لـــا بـــب",
    "تَّ تٌّ ئِ أُ آ ؤ إِ ٱ",
    "كتب، كتاب؛ مكتبة؟ «اقتباس»",
    "ﻻ ﷲ ﺑﺴﻢ",
    "\u{654}ء\u{655}\u{650}\u{651}\u{652}",
];

const PERSIAN_TEXTS: [&str; 4] = [
    "پاکستان زندہ باد",
    "اردو زبان بہت خوبصورت ہے",
    "فارسی شکر است",
    "یہ ایک مثال ہے ۱۲۳",
];

const HEBREW_TEXTS: [&str; 4] = [
    "שָׁלוֹם עוֹלָם",
    "בְּרֵאשִׁית בָּרָא אֱלֹהִים אֵת הַשָּׁמַיִם וְאֵת הָאָרֶץ",
    "יְרוּשָׁלַ\u{5B4}ם וַיְהִי־עֶרֶב",
    "שּׁ שּׂ בּ כּ פּ",
];

const THAI_TEXTS: [&str; 4] = [
    "ภาษาไทย สวัสดีครับ",
    "น้ำ ท่ำ กำ ป่า ปู่ ญู ฐู",
    "ป่ำ ฟ้ำ ปิ่ ฝื้",
    "ก\u{E49}\u{E33} ก\u{E31}\u{E33} ด\u{E4C}\u{E33}",
];

const LAO_TEXTS: [&str; 2] = ["ພາສາລາວ ສະບາຍດີ", "ນ້ຳ ກຳ ໄປ"];

const HANGUL_TEXTS: [&str; 4] = [
    "한국어 텍스트 렌더링",
    "\u{1100}\u{1161}\u{11A8} \u{1112}\u{1161}\u{11AB}\u{1100}\u{1173}\u{11AF}",
    "\u{1100}\u{1176} \u{1100}\u{119E}\u{11AB} \u{AC00}\u{11A8}",
    "\u{AC00}\u{302E} \u{302F}",
];

const CJK_TEXTS: [&str; 4] = [
    "日本語の文章です。カタカナとひらがな。",
    "中文测试，标点符号。“引号”",
    "葛\u{E0100}城市 辻\u{E0101} 邉\u{E0103}",
    "全角ＡＢＣ　１２３",
];

const EMOJI_TEXTS: [&str; 4] = [
    "👨\u{200D}👩\u{200D}👧\u{200D}👦 👩\u{200D}❤\u{FE0F}\u{200D}👨",
    "🇯🇵🇺🇸🇩🇪 1\u{FE0F}\u{20E3} #\u{FE0F}\u{20E3}",
    "👍🏽 🧑🏿\u{200D}🚀 🏳\u{FE0F}\u{200D}🌈 🏴\u{E0067}\u{E0062}\u{E0073}\u{E0063}\u{E0074}\u{E007F}",
    "❤\u{FE0F} ☺\u{FE0E} ⌚ 😀😃😄",
];

const SYRIAC_TEXTS: [&str; 2] = ["ܐܠܗܐ ܫܠܡܐ ܥܠܡܐ", "ܐܵܠܵܗܵܐ ܡܫܝܚܐ"];

const NKO_TEXTS: [&str; 1] = ["ߒߞߏ ߞߊ߲ߜߍ߫ ߓߊ߯ߙߊ"];

const MONGOLIAN_TEXTS: [&str; 2] = ["ᠮᠣᠩᠭᠣᠯ ᠪᠢᠴᠢᠭ", "ᠭ\u{180B}ᠠ ᠨ\u{180C}ᠠ ᠠ\u{180E}ᠠ"];

const PHANTOM_ADVANCE_SAMPLES: [(&str, &str); 5] = [
    ("InterVariable.ttf~HVAR", LATIN_TEXTS[0]),
    ("NotoSans.ttf~HVAR", LATIN_TEXTS[2]),
    ("NotoSerif.ttf~HVAR", LATIN_TEXTS[5]),
    ("NotoSansArabic.ttf~HVAR", ARABIC_TEXTS[1]),
    ("NotoSansDevanagari.ttf~HVAR", "नमस्ते दुनिया र्क क्षि"),
];

const INTER_INSTANCES: [&[(&[u8; 4], f32)]; 5] = [
    &[(b"wght", 100.0)],
    &[(b"wght", 300.0)],
    &[(b"wght", 400.0)],
    &[(b"wght", 650.0), (b"opsz", 28.0)],
    &[(b"wght", 900.0), (b"opsz", 32.0)],
];

pub fn cases() -> Vec<Case> {
    let mut cases = Vec::new();
    for font in [
        "InterVariable.ttf",
        "NotoSans.ttf",
        "NotoSerif.ttf",
        "DejaVuSans.ttf",
    ] {
        for text in LATIN_TEXTS {
            cases.push(Case::new("latin", font, text));
        }
        for text in VIETNAMESE_TEXTS {
            cases.push(Case::new("vietnamese", font, text).language("vi"));
        }
    }
    for features in [
        "liga=0",
        "ss01",
        "tnum",
        "frac",
        "kern=0",
        "smcp",
        "liga[2:9]=0",
        "-calt,+dlig",
    ] {
        cases.push(Case::new("latin", "InterVariable.ttf", LATIN_TEXTS[1]).features(features));
        cases.push(Case::new("latin", "NotoSans.ttf", LATIN_TEXTS[3]).features(features));
    }
    for language in ["tr", "az", "ro", "nl", "ca", "pl", "mo"] {
        cases.push(
            Case::new(
                "latin",
                "NotoSans.ttf",
                "fi ﬁ ij IJ ş ţ ș ț l·l ŀl IJsselmeer",
            )
            .language(language),
        );
    }
    for font in ["NotoSans.ttf", "NotoSerif.ttf", "NotoSans-Italic.ttf"] {
        for text in GREEK_TEXTS {
            cases.push(Case::new("greek", font, text));
        }
        for text in CYRILLIC_TEXTS {
            cases.push(Case::new("cyrillic", font, text));
        }
        for language in ["sr", "mk", "bg", "ru"] {
            cases.push(Case::new("cyrillic", font, CYRILLIC_TEXTS[2]).language(language));
        }
    }
    for variations in INTER_INSTANCES {
        for text in LATIN_TEXTS {
            cases.push(Case::new("variable", "InterVariable.ttf", text).variations(variations));
        }
        cases.push(Case::new("variable", "NotoSans.ttf", LATIN_TEXTS[0]).variations(variations));
    }
    cases.push(
        Case::new("variable", "NotoSans.ttf", LATIN_TEXTS[2])
            .variations(&[(b"wght", 700.0), (b"wdth", 62.5)]),
    );
    cases.push(
        Case::new("variable", "InterVariable-Italic.ttf", LATIN_TEXTS[2])
            .variations(&[(b"wght", 550.0)]),
    );
    for font in [
        "NotoSansArabic.ttf",
        "NotoNaskhArabic.ttf",
        "NotoKufiArabic.ttf",
        "NotoSansArabicUI.ttf",
    ] {
        for text in ARABIC_TEXTS {
            cases.push(Case::new("arabic", font, text));
        }
        cases.push(Case::new("arabic", font, ARABIC_TEXTS[0]).bot_eot());
        cases.push(Case::new("arabic", font, ARABIC_TEXTS[1]).features("-rlig"));
    }
    cases.push(
        Case::new("arabic", "NotoSansArabic.ttf", ARABIC_TEXTS[3]).variations(&[(b"wght", 800.0)]),
    );
    cases.push(
        Case::new("arabic", "NotoKufiArabic.ttf", ARABIC_TEXTS[0]).variations(&[(b"wght", 300.0)]),
    );
    for text in PERSIAN_TEXTS {
        cases.push(Case::new("urdu", "NotoNastaliqUrdu.ttf", text).language("ur"));
        cases.push(Case::new("urdu", "NotoNastaliqUrdu.ttf", text));
        cases.push(Case::new("persian", "NotoNaskhArabic.ttf", text).language("fa"));
    }
    cases.push(Case::new("persian", "NotoSansArabic.ttf", "۱۲۳ ٤٥٦ ۴۵۶").language("ur"));
    cases.push(Case::new("persian", "NotoSansArabic.ttf", "۱۲۳ ٤٥٦ ۴۵۶").language("sd"));
    for font in [
        "NotoSansSyriac.ttf",
        "NotoSansSyriacEastern.ttf",
        "NotoSansSyriacWestern.ttf",
    ] {
        for text in SYRIAC_TEXTS {
            cases.push(Case::new("syriac", font, text));
        }
    }
    for font in ["NotoSansNKo-Regular.otf", "NotoSansNKoUnjoined.ttf"] {
        for text in NKO_TEXTS {
            cases.push(Case::new("nko", font, text));
        }
    }
    for text in MONGOLIAN_TEXTS {
        cases.push(Case::new(
            "mongolian",
            "NotoSansMongolian-Regular.otf",
            text,
        ));
    }
    cases.push(Case::new("joining", "NotoSansAdlam.ttf", "𞤀𞤣𞤤𞤢𞤥 𞤆𞤵𞤤𞤢𞤪"));
    cases.push(Case::new(
        "joining",
        "NotoSansHanifiRohingya.ttf",
        "𐴀𐴁𐴂𐴃 𐴄𐴅𐴆",
    ));
    cases.push(Case::new(
        "joining",
        "NotoSansManichaean-Regular.otf",
        "𐫀𐫁𐫂𐫃 𐫄𐫅",
    ));
    cases.push(Case::new(
        "joining",
        "NotoSansPsalterPahlavi-Regular.otf",
        "𐮀𐮁𐮂𐮃 𐮄𐮅",
    ));
    cases.push(Case::new(
        "joining",
        "NotoSansSogdian-Regular.otf",
        "𐼰𐼱𐼲𐼳 𐼴𐼵",
    ));
    cases.push(Case::new(
        "joining",
        "NotoSansPhagsPa-Regular.otf",
        "ꡀꡁꡂꡃ ꡄꡅ",
    ));
    for font in [
        "NotoSansHebrew.ttf",
        "NotoSerifHebrew.ttf",
        "NotoRashiHebrew.ttf",
    ] {
        for text in HEBREW_TEXTS {
            cases.push(Case::new("hebrew", font, text));
        }
    }
    for font in [
        "NotoSansThai.ttf",
        "NotoSerifThai.ttf",
        "NotoSansThaiLooped.ttf",
    ] {
        for text in THAI_TEXTS {
            cases.push(Case::new("thai", font, text));
        }
    }
    for font in [
        "NotoSansLao.ttf",
        "NotoSerifLao.ttf",
        "NotoSansLaoLooped.ttf",
    ] {
        for text in LAO_TEXTS {
            cases.push(Case::new("lao", font, text));
        }
    }
    for text in HANGUL_TEXTS {
        cases.push(Case::new("hangul", "NotoSansCJK-VF.otf.ttc", text).language("ko"));
        cases.push(Case::new("hangul", "NotoSansCJK-VF.otf.ttc", text).face(1));
    }
    for (face, language) in [
        (0, "ja"),
        (1, "ko"),
        (2, "zh-Hans"),
        (3, "zh-Hant"),
        (4, "zh-HK"),
    ] {
        for text in CJK_TEXTS {
            cases.push(
                Case::new("cjk", "NotoSansCJK-VF.otf.ttc", text)
                    .face(face)
                    .language(language),
            );
        }
    }
    cases.push(
        Case::new("cjk", "NotoSansCJK-VF.otf.ttc", CJK_TEXTS[0]).variations(&[(b"wght", 700.0)]),
    );
    for text in EMOJI_TEXTS {
        cases.push(Case::new("emoji", "NotoColorEmoji.ttf", text));
        cases.push(Case::new("emoji", "NotoSans.ttf", text));
    }
    cases.push(Case::new("latin", "NotoSans.ttf", "\u{301}a \u{308}b").bot_eot());
    cases.push(Case::new(
        "latin",
        "NotoSans.ttf",
        "1\u{2044}2 3\u{2044}4 12\u{2044}345 \u{2044}9",
    ));
    cases.push(Case::new(
        "latin",
        "InterVariable.ttf",
        "1\u{2044}2 3\u{2044}4 12\u{2044}345",
    ));
    cases.push(Case::new(
        "syriac",
        "NotoSansSyriac.ttf",
        "\u{70F}ܐܒܓܕ ܐ\u{70F}ܫܠܡ",
    ));
    cases.push(Case::new(
        "syriac",
        "NotoSansSyriacEastern.ttf",
        "\u{70F}ܐܒܓܕܗܘ",
    ));
    cases.push(Case::new("arabic", "NotoNaskhArabic.ttf", "\u{64E}ب ـــ").bot_eot());
    cases.push(Case::new("mixed", "NotoSans.ttf", "abc ابج 123").script("Latn"));
    cases.push(Case::new("mixed", "NotoSansArabic.ttf", "123 456").script("Arab"));
    cases.push(
        Case::new("mixed", "NotoSansArabic.ttf", "abc (def) [ghi]")
            .script("Arab")
            .rtl(true),
    );
    cases.push(Case::new("mixed", "NotoSansHebrew.ttf", "(שלום) [עולם] {}").rtl(true));
    for (font, text) in PHANTOM_ADVANCE_SAMPLES {
        for variations in INTER_INSTANCES {
            cases.push(Case::new("phantom-advances", font, text).variations(variations));
        }
    }
    syllabic_cases(&mut cases);
    cases
}

const BENCHMARK_LATIN: &str = "The quick brown fox jumps over the lazy dog while the office staff reviews affluent fjord flags. Typography matters: kerning pairs like AV, To, and Wa should look right, ligatures fi and fl should form, and numbers 0123456789 align.";

pub const BENCHMARK_TEXTS: [Case; 19] = [
    Case::new("latin-label", "InterVariable.ttf", "Save changes"),
    Case::new("cjk-label", "NotoSansCJK-VF.otf.ttc", "変更を保存"),
    Case::new("latin", "InterVariable.ttf", BENCHMARK_LATIN),
    Case::new("latin-bold", "InterVariable.ttf", BENCHMARK_LATIN).variations(&[(b"wght", 700.0)]),
    Case::new(
        "latin-serif",
        "NotoSerif.ttf",
        "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat.",
    ),
    Case::new(
        "cyrillic",
        "NotoSans.ttf",
        "Съешь же ещё этих мягких французских булок, да выпей же чаю. Широкая электрификация южных губерний даст мощный толчок подъёму сельского хозяйства.",
    ),
    Case::new(
        "arabic",
        "NotoNaskhArabic.ttf",
        "السلام عليكم ورحمة الله وبركاته. اللغة العربية هي إحدى أكثر اللغات انتشاراً في العالم، يتحدثها أكثر من أربعمائة مليون نسمة.",
    ),
    Case::new(
        "urdu",
        "NotoNastaliqUrdu.ttf",
        "اردو برصغیر کی ایک اہم زبان ہے جو نستعلیق رسم الخط میں لکھی جاتی ہے اور اس کی خوبصورتی مشہور ہے۔",
    ),
    Case::new(
        "hebrew",
        "NotoSansHebrew.ttf",
        "בְּרֵאשִׁית בָּרָא אֱלֹהִים אֵת הַשָּׁמַיִם וְאֵת הָאָרֶץ. וְהָאָרֶץ הָיְתָה תֹהוּ וָבֹהוּ.",
    ),
    Case::new(
        "thai",
        "NotoSansThai.ttf",
        "ภาษาไทยเป็นภาษาที่มีระดับเสียงของคำแน่นอนหรือวรรณยุกต์เช่นเดียวกับภาษาจีน น้ำใจงามและยิ้มแย้มแจ่มใส",
    ),
    Case::new(
        "hangul",
        "NotoSansCJK-VF.otf.ttc",
        "대한민국의 공용어는 한국어이며 한글로 표기한다. 한글은 세종대왕이 창제한 문자로 과학적인 구조를 가지고 있다.",
    ),
    Case::new(
        "cjk",
        "NotoSansCJK-VF.otf.ttc",
        "日本語は主に日本で使用される言語である。漢字、ひらがな、カタカナを組み合わせて表記する。中文是世界上使用人数最多的语言之一。",
    ),
    Case::new(
        "devanagari",
        "NotoSansDevanagari.ttf",
        "हिन्दी भारत की राजभाषा है और विश्व की प्रमुख भाषाओं में से एक है। इसकी लिपि देवनागरी है जिसमें संयुक्ताक्षर, रेफ और मात्राएँ स्वाभाविक रूप से आती हैं।",
    ),
    Case::new(
        "bengali",
        "NotoSansBengali.ttf",
        "বাংলা ভাষা দক্ষিণ এশিয়ার একটি প্রধান ভাষা। এর লিপিতে যুক্তাক্ষর, রেফ এবং কারচিহ্ন সুন্দরভাবে মিলে যায় এবং সাহিত্য অত্যন্ত সমৃদ্ধ।",
    ),
    Case::new(
        "tamil",
        "NotoSansTamil.ttf",
        "தமிழ் உலகின் மிகத் தொன்மையான மொழிகளில் ஒன்றாகும். இது இலங்கை மற்றும் சிங்கப்பூரில் அலுவல் மொழியாக உள்ளது.",
    ),
    Case::new(
        "khmer",
        "NotoSansKhmer.ttf",
        "ភាសាខ្មែរ គឺជាភាសាផ្លូវការរបស់ព្រះរាជាណាចក្រកម្ពុជា ហើយមានអក្សរដែលមានជើងអក្សរ និងស្រៈជាច្រើន។",
    ),
    Case::new(
        "myanmar",
        "NotoSansMyanmar.ttf",
        "မြန်မာဘာသာစကားသည် မြန်မာနိုင်ငံ၏ ရုံးသုံးဘာသာစကားဖြစ်ပြီး သင်္ဘော ကဲ့သို့ ကင်းစီ ပါဝင်သော စကားလုံးများ ရှိသည်။",
    ),
    Case::new(
        "tibetan",
        "NotoSerifTibetan.ttf",
        "བོད་ཀྱི་སྐད་ཡིག་ནི་བོད་ཡུལ་དང་ཧི་མ་ལ་ཡའི་ཁུལ་དུ་བཀོལ་སྤྱོད་བྱེད་པའི་སྐད་ཡིག་ཅིག་ཡིན། བསྒྲུབས་པའི་བརྩེགས་ཡིག་མང་པོ་ཡོད།",
    ),
    Case::new(
        "javanese",
        "NotoSansJavanese.ttf",
        "ꦧꦱꦗꦮ ꦲꦶꦏꦸ ꦧꦱ ꦲꦁꦒꦺꦴꦤ꧀ ꦮꦺꦴꦁ ꦗꦮ ꦲꦶꦁ ꦥꦸꦭꦺꦴ ꦗꦮ꧈ ꦲꦏ꧀ꦱꦫꦤ꧀ꦲꦺ ꦲꦤꦺꦴꦤ꧀ ꦥꦁꦏꦺꦴꦤ꧀ ꦭꦤ꧀ ꦕꦏꦿ꧉",
    ),
];
