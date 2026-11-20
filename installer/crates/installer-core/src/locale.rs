#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Locale {
    pub name: &'static str,
    pub label: &'static str,
    pub english: bool,
}

impl Locale {
    #[must_use]
    pub fn find(name: &str) -> Option<&'static Locale> {
        LOCALES.iter().find(|locale| locale.name == name)
    }
}

macro_rules! locales {
    ($(($name:literal, $label:literal, $english:literal)),* $(,)?) => {
        pub static LOCALES: &[Locale] = &[
            $(Locale { name: $name, label: $label, english: $english }),*
        ];
    };
}

locales![
    ("en_US.UTF-8", "English (United States)", true),
    ("en_GB.UTF-8", "English (United Kingdom)", true),
    ("ja_JP.UTF-8", "Japanese (Japan)", false),
    ("zh_CN.UTF-8", "Chinese (Simplified, China)", false),
    ("zh_TW.UTF-8", "Chinese (Traditional, Taiwan)", false),
    ("ko_KR.UTF-8", "Korean (South Korea)", false),
    ("de_DE.UTF-8", "German (Germany)", false),
    ("fr_FR.UTF-8", "French (France)", false),
    ("es_ES.UTF-8", "Spanish (Spain)", false),
    ("pt_BR.UTF-8", "Portuguese (Brazil)", false),
    ("it_IT.UTF-8", "Italian (Italy)", false),
    ("nl_NL.UTF-8", "Dutch (Netherlands)", false),
    ("pl_PL.UTF-8", "Polish (Poland)", false),
    ("ru_RU.UTF-8", "Russian (Russia)", false),
    ("uk_UA.UTF-8", "Ukrainian (Ukraine)", false),
    ("tr_TR.UTF-8", "Turkish (Turkey)", false),
    ("sv_SE.UTF-8", "Swedish (Sweden)", false),
    ("nb_NO.UTF-8", "Norwegian Bokmal (Norway)", false),
    ("da_DK.UTF-8", "Danish (Denmark)", false),
    ("fi_FI.UTF-8", "Finnish (Finland)", false),
    ("cs_CZ.UTF-8", "Czech (Czechia)", false),
    ("hu_HU.UTF-8", "Hungarian (Hungary)", false),
    ("id_ID.UTF-8", "Indonesian (Indonesia)", false),
    ("vi_VN.UTF-8", "Vietnamese (Vietnam)", false),
    ("th_TH.UTF-8", "Thai (Thailand)", false),
    ("ar_EG.UTF-8", "Arabic (Egypt)", false),
    ("he_IL.UTF-8", "Hebrew (Israel)", false),
    ("hi_IN.UTF-8", "Hindi (India)", false),
];
