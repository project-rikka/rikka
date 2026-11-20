#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Keyboard {
    pub label: &'static str,
    pub keymap: &'static str,
    pub layout: &'static str,
    pub variant: &'static str,
}

impl Keyboard {
    #[must_use]
    pub fn find(label: &str) -> Option<&'static Keyboard> {
        KEYBOARDS.iter().find(|keyboard| keyboard.label == label)
    }
}

macro_rules! keyboards {
    ($(($label:literal, $keymap:literal, $layout:literal, $variant:literal)),* $(,)?) => {
        pub static KEYBOARDS: &[Keyboard] = &[
            $(Keyboard { label: $label, keymap: $keymap, layout: $layout, variant: $variant }),*
        ];
    };
}

keyboards![
    ("English (US)", "us", "us", ""),
    ("English (US, Dvorak)", "dvorak", "us", "dvorak"),
    ("English (US, Colemak)", "colemak", "us", "colemak"),
    ("English (UK)", "uk", "gb", ""),
    ("Japanese (JIS 106)", "jp106", "jp", ""),
    ("Korean", "us", "kr", "kr104"),
    ("German", "de", "de", ""),
    ("German (Switzerland)", "de_CH-latin1", "ch", "de"),
    ("French", "fr", "fr", ""),
    ("French (BEPO)", "fr-bepo", "fr", "bepo"),
    ("French (Canada)", "cf", "ca", ""),
    ("Spanish", "es", "es", ""),
    ("Spanish (Latin America)", "la-latin1", "latam", ""),
    ("Portuguese (Brazil, ABNT2)", "br-abnt2", "br", ""),
    ("Portuguese (Portugal)", "pt-latin1", "pt", ""),
    ("Italian", "it", "it", ""),
    ("Dutch", "nl", "nl", ""),
    ("Norwegian", "no", "no", ""),
    ("Swedish", "sv-latin1", "se", ""),
    ("Danish", "dk-latin1", "dk", ""),
    ("Finnish", "fi", "fi", ""),
    ("Polish", "pl", "pl", ""),
    ("Czech", "cz-lat2", "cz", ""),
    ("Hungarian", "hu", "hu", ""),
    ("Turkish (Q)", "trq", "tr", ""),
    ("Russian", "ru", "ru", ""),
    ("Ukrainian", "ua-utf", "ua", ""),
    ("Greek", "gr", "gr", ""),
    ("Chinese (Pinyin)", "us", "cn", ""),
];
