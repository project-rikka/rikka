use rikka_text_font::Tag;
use rikka_text_unicode::Script;

use crate::shapers::arabic::shaper::ARABIC_SHAPER;
use crate::shapers::complex_shaper::ComplexShaper;
use crate::shapers::default::DEFAULT_SHAPER;
use crate::shapers::hangul::HANGUL_SHAPER;
use crate::shapers::hebrew::HEBREW_SHAPER;
use crate::shapers::indic::shaper::INDIC_SHAPER;
use crate::shapers::khmer::shaper::KHMER_SHAPER;
use crate::shapers::myanmar::shaper::MYANMAR_SHAPER;
use crate::shapers::thai::THAI_SHAPER;
use crate::shapers::universal::shaper::UNIVERSAL_SHAPER;

const DEFAULT_SCRIPT: Tag = Tag::new(b"DFLT");
const LATIN_SCRIPT: Tag = Tag::new(b"latn");
const MYANMAR_LEGACY_SCRIPT: Tag = Tag::new(b"mymr");
const UNIVERSAL_SPEC_SUFFIX: u8 = b'3';

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Route {
    Default,
    Arabic,
    Hangul,
    Hebrew,
    Thai,
    Indic,
    Khmer,
    Myanmar,
    Universal,
}

fn route(script: Script) -> Route {
    match script {
        Script::Arabic | Script::Syriac => Route::Arabic,
        Script::Thai | Script::Lao => Route::Thai,
        Script::Hangul => Route::Hangul,
        Script::Hebrew => Route::Hebrew,
        Script::Bengali
        | Script::Devanagari
        | Script::Gujarati
        | Script::Gurmukhi
        | Script::Kannada
        | Script::Malayalam
        | Script::Oriya
        | Script::Tamil
        | Script::Telugu => Route::Indic,
        Script::Khmer => Route::Khmer,
        Script::Myanmar => Route::Myanmar,
        Script::Tibetan
        | Script::Mongolian
        | Script::Sinhala
        | Script::Buhid
        | Script::Hanunoo
        | Script::Tagalog
        | Script::Tagbanwa
        | Script::Limbu
        | Script::TaiLe
        | Script::Buginese
        | Script::Kharoshthi
        | Script::SylotiNagri
        | Script::Tifinagh
        | Script::Balinese
        | Script::Nko
        | Script::PhagsPa
        | Script::Cham
        | Script::KayahLi
        | Script::Lepcha
        | Script::Rejang
        | Script::Saurashtra
        | Script::Sundanese
        | Script::EgyptianHieroglyphs
        | Script::Javanese
        | Script::Kaithi
        | Script::MeeteiMayek
        | Script::TaiTham
        | Script::TaiViet
        | Script::Batak
        | Script::Brahmi
        | Script::Mandaic
        | Script::Chakma
        | Script::Miao
        | Script::Sharada
        | Script::Takri
        | Script::Duployan
        | Script::Grantha
        | Script::Khojki
        | Script::Khudawadi
        | Script::Mahajani
        | Script::Manichaean
        | Script::Modi
        | Script::PahawhHmong
        | Script::PsalterPahlavi
        | Script::Siddham
        | Script::Tirhuta
        | Script::Ahom
        | Script::Multani
        | Script::Adlam
        | Script::Bhaiksuki
        | Script::Marchen
        | Script::Newa
        | Script::MasaramGondi
        | Script::Soyombo
        | Script::ZanabazarSquare
        | Script::Dogra
        | Script::GunjalaGondi
        | Script::HanifiRohingya
        | Script::Makasar
        | Script::Medefaidrin
        | Script::OldSogdian
        | Script::Sogdian
        | Script::Elymaic
        | Script::Nandinagari
        | Script::NyiakengPuachueHmong
        | Script::Wancho
        | Script::Chorasmian
        | Script::DivesAkuru
        | Script::KhitanSmallScript
        | Script::Yezidi
        | Script::CyproMinoan
        | Script::OldUyghur
        | Script::Tangsa
        | Script::Toto
        | Script::Vithkuqi
        | Script::Kawi
        | Script::NagMundari
        | Script::Garay
        | Script::GurungKhema
        | Script::KiratRai
        | Script::OlOnal
        | Script::Sunuwar
        | Script::Todhri
        | Script::TuluTigalari
        | Script::BeriaErfe
        | Script::Sidetic
        | Script::TaiYo
        | Script::TolongSiki => Route::Universal,
        _ => Route::Default,
    }
}

fn designed_for_default(gsub_script: Option<Tag>) -> bool {
    gsub_script == Some(DEFAULT_SCRIPT) || gsub_script == Some(LATIN_SCRIPT)
}

fn uses_universal_spec(gsub_script: Option<Tag>) -> bool {
    gsub_script.is_some_and(|tag| tag.to_bytes()[3] == UNIVERSAL_SPEC_SUFFIX)
}

pub(crate) fn select(script: Script, gsub_script: Option<Tag>) -> &'static dyn ComplexShaper {
    match route(script) {
        Route::Arabic if gsub_script != Some(DEFAULT_SCRIPT) || script == Script::Arabic => {
            &ARABIC_SHAPER
        }
        Route::Thai => &THAI_SHAPER,
        Route::Hangul => &HANGUL_SHAPER,
        Route::Hebrew => &HEBREW_SHAPER,
        Route::Khmer => &KHMER_SHAPER,
        Route::Indic if designed_for_default(gsub_script) => &DEFAULT_SHAPER,
        Route::Indic if uses_universal_spec(gsub_script) => &UNIVERSAL_SHAPER,
        Route::Indic => &INDIC_SHAPER,
        Route::Myanmar
            if designed_for_default(gsub_script) || gsub_script == Some(MYANMAR_LEGACY_SCRIPT) =>
        {
            &DEFAULT_SHAPER
        }
        Route::Myanmar => &MYANMAR_SHAPER,
        Route::Universal if !designed_for_default(gsub_script) => &UNIVERSAL_SHAPER,
        Route::Default | Route::Arabic | Route::Universal => &DEFAULT_SHAPER,
    }
}
