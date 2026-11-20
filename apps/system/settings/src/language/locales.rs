use rikka_app::prelude::*;

const LOCALES: [(&str, &str); 28] = [
    ("en_US.UTF-8", "English (United States)"),
    ("en_GB.UTF-8", "English (United Kingdom)"),
    ("ja_JP.UTF-8", "日本語 (日本)"),
    ("zh_CN.UTF-8", "简体中文 (中国)"),
    ("zh_TW.UTF-8", "繁體中文 (台灣)"),
    ("ko_KR.UTF-8", "한국어 (대한민국)"),
    ("de_DE.UTF-8", "Deutsch (Deutschland)"),
    ("fr_FR.UTF-8", "Français (France)"),
    ("es_ES.UTF-8", "Español (España)"),
    ("pt_BR.UTF-8", "Português (Brasil)"),
    ("it_IT.UTF-8", "Italiano (Italia)"),
    ("nl_NL.UTF-8", "Nederlands (Nederland)"),
    ("pl_PL.UTF-8", "Polski (Polska)"),
    ("ru_RU.UTF-8", "Русский (Россия)"),
    ("uk_UA.UTF-8", "Українська (Україна)"),
    ("tr_TR.UTF-8", "Türkçe (Türkiye)"),
    ("sv_SE.UTF-8", "Svenska (Sverige)"),
    ("nb_NO.UTF-8", "Norsk bokmål (Norge)"),
    ("da_DK.UTF-8", "Dansk (Danmark)"),
    ("fi_FI.UTF-8", "Suomi (Suomi)"),
    ("cs_CZ.UTF-8", "Čeština (Česko)"),
    ("hu_HU.UTF-8", "Magyar (Magyarország)"),
    ("id_ID.UTF-8", "Bahasa Indonesia (Indonesia)"),
    ("vi_VN.UTF-8", "Tiếng Việt (Việt Nam)"),
    ("th_TH.UTF-8", "ไทย (ไทย)"),
    ("ar_EG.UTF-8", "العربية (مصر)"),
    ("he_IL.UTF-8", "עברית (ישראל)"),
    ("hi_IN.UTF-8", "हिन्दी (भारत)"),
];

pub(crate) fn choices() -> Vec<(String, Label)> {
    LOCALES
        .iter()
        .map(|(name, label)| ((*name).to_owned(), Label::from(*label)))
        .collect()
}
