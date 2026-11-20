use rikka_languages::Language;
use whatlang::Lang;

use crate::message::PageReport;

const SIMPLIFIED_CHINESE: &str = "zh-Hans";
const TRADITIONAL_CHINESE: &str = "zh-Hant";
const TRADITIONAL_ONLY: &str = "這們說個來對時會國過還發後學問開關門見長東車馬鳥魚電語讀書寫聽觀點體術機無與從當爲為將應經樣麼內實際動現產業線縣頭給讓認識進運並號環區連續邊員選舉價錢歡廣場灣義網傳統";
const SIMPLIFIED_ONLY: &str = "这们说个来对时会国过还发后学问开关门见长东车马鸟鱼电语读书写听观点体术机无与从当为将应经样么内实际动现产业线县头给让认识进运并号环区连续边员选举价钱欢广场湾义网传统";

#[must_use]
pub fn detect(report: &PageReport) -> Option<Language> {
    let hint = report.lang.as_deref().and_then(Language::lookup);
    let detected = whatlang::detect(&report.sample)
        .filter(whatlang::Info::is_reliable)
        .and_then(|info| language_of(info.lang(), &report.sample, hint.as_ref()));
    detected.or(hint)
}

fn language_of(lang: Lang, sample: &str, hint: Option<&Language>) -> Option<Language> {
    let tag = match lang {
        Lang::Cmn => chinese(sample, hint),
        lang => tag_of(lang),
    };
    Language::from_tag(tag)
}

fn chinese(sample: &str, hint: Option<&Language>) -> &'static str {
    let count = |set: &str| sample.chars().filter(|c| set.contains(*c)).count();
    let traditional = count(TRADITIONAL_ONLY);
    let simplified = count(SIMPLIFIED_ONLY);
    let hinted = hint.is_some_and(|hint| hint.tag() == TRADITIONAL_CHINESE);
    if traditional > simplified || (traditional == simplified && hinted) {
        TRADITIONAL_CHINESE
    } else {
        SIMPLIFIED_CHINESE
    }
}

fn tag_of(lang: Lang) -> &'static str {
    match lang {
        Lang::Epo => "eo",
        Lang::Eng => "en",
        Lang::Rus => "ru",
        Lang::Cmn => SIMPLIFIED_CHINESE,
        Lang::Spa => "es",
        Lang::Por => "pt",
        Lang::Ita => "it",
        Lang::Ben => "bn",
        Lang::Fra => "fr",
        Lang::Deu => "de",
        Lang::Ukr => "uk",
        Lang::Kat => "ka",
        Lang::Ara => "ar",
        Lang::Hin => "hi",
        Lang::Jpn => "ja",
        Lang::Heb => "he",
        Lang::Yid => "yi",
        Lang::Pol => "pl",
        Lang::Amh => "am",
        Lang::Jav => "jv",
        Lang::Kor => "ko",
        Lang::Nob => "nb",
        Lang::Dan => "da",
        Lang::Swe => "sv",
        Lang::Fin => "fi",
        Lang::Tur => "tr",
        Lang::Nld => "nl",
        Lang::Hun => "hu",
        Lang::Ces => "cs",
        Lang::Ell => "el",
        Lang::Bul => "bg",
        Lang::Bel => "be",
        Lang::Mar => "mr",
        Lang::Kan => "kn",
        Lang::Ron => "ro",
        Lang::Slv => "sl",
        Lang::Hrv => "hr",
        Lang::Srp => "sr",
        Lang::Mkd => "mk",
        Lang::Lit => "lt",
        Lang::Lav => "lv",
        Lang::Est => "et",
        Lang::Tam => "ta",
        Lang::Vie => "vi",
        Lang::Urd => "ur",
        Lang::Tha => "th",
        Lang::Guj => "gu",
        Lang::Uzb => "uz",
        Lang::Pan => "pa",
        Lang::Aze => "az",
        Lang::Ind => "id",
        Lang::Tel => "te",
        Lang::Pes => "fa",
        Lang::Mal => "ml",
        Lang::Ori => "or",
        Lang::Mya => "my",
        Lang::Nep => "ne",
        Lang::Sin => "si",
        Lang::Khm => "km",
        Lang::Tuk => "tk",
        Lang::Aka => "ak",
        Lang::Zul => "zu",
        Lang::Sna => "sn",
        Lang::Afr => "af",
        Lang::Lat => "la",
        Lang::Slk => "sk",
        Lang::Cat => "ca",
        Lang::Tgl => "fil",
        Lang::Hye => "hy",
        Lang::Cym => "cy",
    }
}
