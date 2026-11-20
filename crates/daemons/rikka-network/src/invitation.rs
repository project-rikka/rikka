use std::fmt;
use std::str::FromStr;

use crate::error::Error;
use crate::security::Security;
use crate::ssid::Ssid;

const SCHEME: &str = "WIFI:";
const LONGEST_NAME: usize = 32;
const ESCAPE: char = '\\';
const QUOTE: char = '"';
const WPA3_ONLY: u32 = 0x1;
const WHAT: &str = "a shared network";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Invitation {
    pub ssid: Ssid,
    pub security: Security,
    pub passphrase: String,
    pub hidden: bool,
}

impl fmt::Display for Invitation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{SCHEME}T:{};S:{};",
            kind(self.security),
            escaped(&self.ssid.name())
        )?;
        if !self.passphrase.is_empty() {
            write!(f, "P:{};", escaped(&self.passphrase))?;
        }
        if self.hidden {
            f.write_str("H:true;")?;
        }
        f.write_str(";")
    }
}

impl FromStr for Invitation {
    type Err = Error;

    fn from_str(text: &str) -> Result<Invitation, Error> {
        let text = text.trim();
        let body = text
            .get(..SCHEME.len())
            .filter(|scheme| scheme.eq_ignore_ascii_case(SCHEME))
            .map(|_| &text[SCHEME.len()..])
            .ok_or_else(|| Error::invalid(WHAT, "it does not start with WIFI:"))?;
        let fields = Fields::parse(body);

        let name = fields.get("S").unwrap_or_default();
        if name.is_empty() {
            return Err(Error::invalid(WHAT, "it names no network"));
        }
        if name.len() > LONGEST_NAME {
            return Err(Error::invalid(
                WHAT,
                format!(
                    "its name is {} bytes long, more than {LONGEST_NAME}",
                    name.len()
                ),
            ));
        }
        let passphrase = fields.get("P").unwrap_or_default();
        let security = security(
            fields.get("T").as_deref(),
            fields.get("R").as_deref(),
            !passphrase.is_empty(),
        )?;
        if security.needs_passphrase() && passphrase.is_empty() {
            return Err(Error::invalid(WHAT, "it gives no password"));
        }
        let hidden = fields
            .get("H")
            .is_some_and(|hidden| hidden.eq_ignore_ascii_case("true"));

        Ok(Invitation {
            ssid: Ssid::new(name.into_bytes()),
            security,
            passphrase,
            hidden,
        })
    }
}

struct Fields<'a>(Vec<(&'a str, &'a str)>);

impl<'a> Fields<'a> {
    fn parse(body: &'a str) -> Fields<'a> {
        let mut fields = Vec::new();
        let mut rest = body;
        while !rest.is_empty() {
            let end = unescaped(rest, ';').unwrap_or(rest.len());
            let field = &rest[..end];
            rest = rest.get(end + 1..).unwrap_or_default();
            if field.is_empty() {
                break;
            }
            if let Some((key, value)) = field.split_once(':') {
                fields.push((key, value));
            }
        }
        Fields(fields)
    }

    fn get(&self, field_key: &str) -> Option<String> {
        self.0
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(field_key))
            .map(|(_, value)| decoded(value))
    }
}

fn unescaped(text: &str, delimiter: char) -> Option<usize> {
    let mut escaped = false;
    for (at, character) in text.char_indices() {
        match (escaped, character) {
            (true, _) => escaped = false,
            (false, ESCAPE) => escaped = true,
            (false, found) if found == delimiter => return Some(at),
            _ => {}
        }
    }
    None
}

fn decoded(raw: &str) -> String {
    let quoted = raw.len() >= 2
        && raw.starts_with(QUOTE)
        && unescaped(&raw[1..], QUOTE) == Some(raw.len() - 2);
    let raw = match quoted {
        true => &raw[1..raw.len() - 1],
        false => raw,
    };
    let mut decoded = String::with_capacity(raw.len());
    let mut characters = raw.chars();
    while let Some(character) = characters.next() {
        match character {
            ESCAPE => decoded.extend(characters.next()),
            other => decoded.push(other),
        }
    }
    decoded
}

fn security(
    kind: Option<&str>,
    disabled: Option<&str>,
    has_passphrase: bool,
) -> Result<Security, Error> {
    let kind = kind.unwrap_or_default().trim().to_ascii_uppercase();
    let wpa3_only = disabled
        .and_then(|bits| u32::from_str_radix(bits.trim(), 16).ok())
        .is_some_and(|bits| bits & WPA3_ONLY != 0);
    Ok(match kind.as_str() {
        "" if has_passphrase => Security::Personal,
        "" | "NOPASS" => Security::Open,
        "WEP" => Security::Wep,
        "WPA" | "WPA2" | "WPA/WPA2" | "WPA2/WPA3" if wpa3_only => Security::Wpa3Personal,
        "WPA" | "WPA2" | "WPA/WPA2" | "WPA2/WPA3" => Security::Personal,
        "SAE" | "WPA3" => Security::Wpa3Personal,
        "WPA-EAP" | "WPA2-EAP" | "WPA3-EAP" | "EAP" => Security::Enterprise,
        unknown => {
            return Err(Error::invalid(
                WHAT,
                format!("unsupported security `{unknown}`"),
            ));
        }
    })
}

fn kind(security: Security) -> &'static str {
    match security {
        Security::Open | Security::Enhanced => "nopass",
        Security::Wep => "WEP",
        Security::Wpa3Personal => "SAE",
        Security::Personal | Security::Enterprise | Security::Wpa3Enterprise => "WPA",
    }
}

fn escaped(value: &str) -> String {
    let escaped: String = value
        .chars()
        .flat_map(|character| {
            let escape = matches!(character, ESCAPE | ';' | ',' | ':' | QUOTE);
            escape.then_some(ESCAPE).into_iter().chain(Some(character))
        })
        .collect();
    if !value.is_empty()
        && value.len().is_multiple_of(2)
        && value.chars().all(|character| character.is_ascii_hexdigit())
    {
        return format!("{QUOTE}{escaped}{QUOTE}");
    }
    escaped
}
