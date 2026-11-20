use rikka_proton_contentline::{Parameter, escape_text, split_unescaped, unescape_text};

use crate::property::Property;
use crate::quoted_printable::decode_quoted_printable_value;
use crate::vcard::VCard;
use crate::version::Version;

const TYPE: &str = "TYPE";
const PREF: &str = "PREF";
const ENCODING: &str = "ENCODING";
const CHARSET: &str = "CHARSET";
const VALUE: &str = "VALUE";
const BINARY: [&str; 4] = ["PHOTO", "LOGO", "SOUND", "KEY"];
const ENCODINGS: [&str; 5] = ["QUOTED-PRINTABLE", "BASE64", "B", "8BIT", "7BIT"];
const STRUCTURED: [&str; 4] = ["N", "ADR", "ORG", "GENDER"];
const LISTS: [&str; 2] = ["CATEGORIES", "NICKNAME"];
const UNESCAPED: [&str; 14] = [
    "URL",
    "PHOTO",
    "LOGO",
    "SOUND",
    "KEY",
    "GEO",
    "UID",
    "SOURCE",
    "FBURL",
    "CALURI",
    "CALADRURI",
    "IMPP",
    "MEMBER",
    "RELATED",
];

impl VCard {
    #[must_use]
    pub fn to_v4(&self) -> VCard {
        if *self.version() == Version::V4 {
            return self.clone();
        }

        let source = match self.version() {
            Version::V2_1 => VcardVersion::V21,
            _ => VcardVersion::V3OrLater,
        };
        let mut upgraded = VCard::with_version(Version::V4);
        for property in self.properties() {
            upgraded.push(upgrade_property(property.clone(), source));
        }
        upgraded
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum VcardVersion {
    V21,
    V3OrLater,
}

fn upgrade_property(mut property: Property, source: VcardVersion) -> Property {
    let charset = property.parameter_value(CHARSET).map(str::to_owned);
    property.remove_parameter(CHARSET);

    let mut types: Vec<String> = property
        .parameter(TYPE)
        .map(|parameter| parameter.values().to_vec())
        .unwrap_or_default();
    let preferred = types.iter().any(|kind| kind.eq_ignore_ascii_case(PREF));
    types.retain(|kind| !kind.eq_ignore_ascii_case(PREF));

    let mut encoding = property
        .parameter_value(ENCODING)
        .map(str::to_ascii_uppercase);
    if let Some(bare) = types.iter().position(|kind| {
        ENCODINGS
            .iter()
            .any(|known| kind.eq_ignore_ascii_case(known))
    }) {
        let bare = types.remove(bare).to_ascii_uppercase();
        encoding = encoding.or(Some(bare));
    }
    types.retain(|kind| {
        !ENCODINGS
            .iter()
            .any(|known| kind.eq_ignore_ascii_case(known))
    });

    match encoding.as_deref() {
        Some("QUOTED-PRINTABLE") => {
            let text = decode_quoted_printable_value(property.raw_value(), charset.as_deref());
            let raw = match source {
                VcardVersion::V21 => reescape_v2_1_value(property.name(), &text),
                VcardVersion::V3OrLater => text,
            };
            property.set_raw_value(raw);
            property.remove_parameter(ENCODING);
        }
        Some("8BIT" | "7BIT") => {
            property.remove_parameter(ENCODING);
            if source == VcardVersion::V21 {
                let raw = reescape_v2_1_value(property.name(), property.raw_value());
                property.set_raw_value(raw);
            }
        }
        Some("B" | "BASE64") if BINARY.iter().any(|name| property.is(name)) => {
            let media = media_type(property.name(), &types);
            types.retain(|kind| !is_format_type(kind));
            let payload: String = property
                .raw_value()
                .chars()
                .filter(|character| !character.is_whitespace())
                .collect();
            property.set_raw_value(format!("data:{media};base64,{payload}"));
            property.remove_parameter(ENCODING);
            property.remove_parameter(VALUE);
        }
        _ if source == VcardVersion::V21 => {
            let raw = reescape_v2_1_value(property.name(), property.raw_value());
            property.set_raw_value(raw);
        }
        _ => {}
    }

    property.remove_parameter(TYPE);
    if !types.is_empty() {
        property.set_parameter(Parameter::with_values(TYPE, types));
    }
    if preferred && property.pref().is_none() {
        property.set_pref(1);
    }

    property
}

fn media_type(name: &str, types: &[String]) -> String {
    let format = types.iter().find(|kind| is_format_type(kind));
    match (name.to_ascii_uppercase().as_str(), format) {
        ("KEY", Some(format)) if format.eq_ignore_ascii_case("pgp") => {
            "application/pgp-keys".to_owned()
        }
        ("KEY", Some(format)) => format!("application/{}", format.to_ascii_lowercase()),
        ("KEY", None) => "application/pgp-keys".to_owned(),
        ("SOUND", Some(format)) => format!("audio/{}", format.to_ascii_lowercase()),
        ("SOUND", None) => "audio/basic".to_owned(),
        (_, Some(format)) if format.contains('/') => format.to_ascii_lowercase(),
        (_, Some(format)) if format.eq_ignore_ascii_case("jpg") => "image/jpeg".to_owned(),
        (_, Some(format)) => format!("image/{}", format.to_ascii_lowercase()),
        (_, None) => "image/jpeg".to_owned(),
    }
}

fn is_format_type(kind: &str) -> bool {
    matches!(
        kind.to_ascii_lowercase().as_str(),
        "jpeg"
            | "jpg"
            | "png"
            | "gif"
            | "bmp"
            | "tiff"
            | "webp"
            | "pgp"
            | "x509"
            | "wave"
            | "pcm"
            | "aiff"
            | "mp3"
            | "ogg"
    ) || kind.contains('/')
}

fn reescape_v2_1_value(name: &str, value: &str) -> String {
    let is_one_of = |names: &[&str]| names.iter().any(|known| known.eq_ignore_ascii_case(name));
    if is_one_of(&UNESCAPED) {
        return value.to_owned();
    }
    let separator = match (is_one_of(&STRUCTURED), is_one_of(&LISTS)) {
        (true, _) => Some(';'),
        (_, true) => Some(','),
        _ => None,
    };
    match separator {
        Some(separator) => split_unescaped(value, separator)
            .into_iter()
            .map(|part| escape_text(&unescape_text(part)))
            .collect::<Vec<_>>()
            .join(&separator.to_string()),
        None => escape_text(&unescape_text(value)),
    }
}
