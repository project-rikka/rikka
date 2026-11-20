use std::collections::HashMap;
use std::fmt;
use std::io::Read;
use std::ops::Range;

use rikka_types::CancelToken;
use serde::Deserialize;
use serde::de::value::MapAccessDeserializer;
use serde::de::{self, DeserializeSeed, Deserializer, IgnoredAny, MapAccess, SeqAccess, Visitor};

use crate::detail::Detail;
use crate::error::{Error, Result};
use crate::license::License;
use crate::maintainer::Maintainer;
use crate::team::Team;

const PACKAGES: &str = "packages";
const CANCELLED: &str = "cancelled";
const CHECKED_EVERY: usize = 4096;

#[derive(Debug)]
pub(crate) struct Record {
    pub(crate) attribute: String,
    pub(crate) name: String,
    pub(crate) version: String,
    pub(crate) description: String,
    pub(crate) unfree: bool,
    pub(crate) detail: Range<usize>,
}

pub(crate) struct Imported {
    pub(crate) records: Vec<Record>,
    pub(crate) details: Vec<u8>,
    pub(crate) platforms: Vec<String>,
}

pub(crate) fn read(reader: impl Read, cancel: &CancelToken) -> Result<Imported> {
    let mut importer = Importer {
        imported: Imported {
            records: Vec::new(),
            details: Vec::new(),
            platforms: Vec::new(),
        },
        known: HashMap::new(),
        cancel,
    };
    let mut deserializer = serde_json::Deserializer::from_reader(reader);
    let read = Index(&mut importer)
        .deserialize(&mut deserializer)
        .and_then(|catalog| deserializer.end().map(|()| catalog));
    if cancel.is_cancelled() {
        return Err(Error::Cancelled);
    }
    match read {
        Ok(true) => Ok(importer.imported),
        Ok(false) => Err(Error::EmptyIndex),
        Err(error) => Err(Error::Index(error)),
    }
}

struct Importer<'a> {
    imported: Imported,
    known: HashMap<String, u16>,
    cancel: &'a CancelToken,
}

impl Importer<'_> {
    fn add(&mut self, attribute: String, raw: Raw) -> Result<(), String> {
        let meta = raw.meta.unwrap_or_default();
        let platforms = meta
            .platforms
            .map(|listed| listed.0)
            .unwrap_or_default()
            .into_iter()
            .map(|platform| self.platform(platform))
            .collect::<Result<Vec<u16>, String>>()?;
        let detail = Detail {
            about: meta.long_description.unwrap_or_default(),
            main_program: meta.main_program.unwrap_or_default(),
            outputs: raw.outputs.map(|outputs| outputs.0).unwrap_or_default(),
            platforms,
            licenses: meta.license.map(|licenses| licenses.0).unwrap_or_default(),
            maintainers: meta
                .maintainers
                .unwrap_or_default()
                .into_iter()
                .map(Person::into_maintainer)
                .collect(),
            teams: meta
                .teams
                .unwrap_or_default()
                .into_iter()
                .map(Group::into_team)
                .collect(),
            homepages: meta.homepage.map(|pages| pages.0).unwrap_or_default(),
            position: meta.position.unwrap_or_default(),
        };

        let details = &mut self.imported.details;
        let at = details.len();
        serde_json::to_writer(&mut *details, &detail).map_err(|error| error.to_string())?;
        let detail = at..details.len();

        self.imported.records.push(Record {
            attribute,
            name: raw.pname.unwrap_or_default(),
            version: raw.version.unwrap_or_default(),
            description: meta.description.unwrap_or_default(),
            unfree: meta.unfree.unwrap_or_default(),
            detail,
        });
        Ok(())
    }

    fn platform(&mut self, platform: String) -> Result<u16, String> {
        if let Some(known) = self.known.get(&platform) {
            return Ok(*known);
        }
        let index =
            u16::try_from(self.imported.platforms.len()).map_err(|error| error.to_string())?;
        self.imported.platforms.push(platform.clone());
        self.known.insert(platform, index);
        Ok(index)
    }
}

struct Index<'a, 'b>(&'a mut Importer<'b>);

impl<'de> DeserializeSeed<'de> for Index<'_, '_> {
    type Value = bool;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<bool, D::Error> {
        deserializer.deserialize_map(self)
    }
}

impl<'de> Visitor<'de> for Index<'_, '_> {
    type Value = bool;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("the packages.json a NixOS channel publishes")
    }

    fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<bool, M::Error> {
        let mut has_packages = false;
        while let Some(key) = map.next_key::<String>()? {
            if key == PACKAGES {
                map.next_value_seed(Packages(&mut *self.0))?;
                has_packages = true;
            } else {
                map.next_value::<IgnoredAny>()?;
            }
        }
        Ok(has_packages)
    }
}

struct Packages<'a, 'b>(&'a mut Importer<'b>);

impl<'de> DeserializeSeed<'de> for Packages<'_, '_> {
    type Value = ();

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<(), D::Error> {
        deserializer.deserialize_map(self)
    }
}

impl<'de> Visitor<'de> for Packages<'_, '_> {
    type Value = ();

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("packages by attribute path")
    }

    fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<(), M::Error> {
        let mut read = 0usize;
        while let Some(attribute) = map.next_key::<String>()? {
            let raw: Raw = map.next_value()?;
            self.0.add(attribute, raw).map_err(de::Error::custom)?;
            read += 1;
            if read.is_multiple_of(CHECKED_EVERY) && self.0.cancel.is_cancelled() {
                return Err(de::Error::custom(CANCELLED));
            }
        }
        Ok(())
    }
}

#[derive(Deserialize)]
struct Raw {
    #[serde(default)]
    pname: Option<String>,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    outputs: Option<Keys>,
    #[serde(default)]
    meta: Option<Meta>,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Meta {
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    long_description: Option<String>,
    #[serde(default)]
    unfree: Option<bool>,
    #[serde(default)]
    main_program: Option<String>,
    #[serde(default)]
    license: Option<Licenses>,
    #[serde(default)]
    maintainers: Option<Vec<Person>>,
    #[serde(default)]
    teams: Option<Vec<Group>>,
    #[serde(default)]
    homepage: Option<Texts>,
    #[serde(default)]
    platforms: Option<Texts>,
    #[serde(default)]
    position: Option<String>,
}

#[derive(Deserialize)]
struct Person {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    github: Option<String>,
    #[serde(default)]
    email: Option<String>,
}

impl Person {
    fn into_maintainer(self) -> Maintainer {
        Maintainer {
            name: self.name.unwrap_or_default(),
            github: self.github.unwrap_or_default(),
            email: self.email.unwrap_or_default(),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Group {
    #[serde(default)]
    short_name: Option<String>,
    #[serde(default)]
    github: Option<String>,
    #[serde(default)]
    scope: Option<String>,
}

impl Group {
    fn into_team(self) -> Team {
        Team {
            name: self.short_name.or(self.github).unwrap_or_default(),
            scope: self.scope.unwrap_or_default(),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawLicense {
    #[serde(default)]
    full_name: Option<String>,
    #[serde(default)]
    short_name: Option<String>,
    #[serde(default)]
    spdx_id: Option<String>,
    #[serde(default)]
    url: Option<String>,
}

impl RawLicense {
    fn into_license(self) -> License {
        License {
            name: self.full_name.or(self.short_name).unwrap_or_default(),
            spdx: self.spdx_id.unwrap_or_default(),
            url: self.url.unwrap_or_default(),
        }
    }
}

fn named(name: &str) -> License {
    License {
        name: name.to_owned(),
        ..License::default()
    }
}

fn drained_map<'de, M: MapAccess<'de>>(mut map: M) -> Result<(), M::Error> {
    while map.next_entry::<IgnoredAny, IgnoredAny>()?.is_some() {}
    Ok(())
}

fn drained_seq<'de, S: SeqAccess<'de>>(mut seq: S) -> Result<(), S::Error> {
    while seq.next_element::<IgnoredAny>()?.is_some() {}
    Ok(())
}

macro_rules! ignoring_scalars {
    ($value:expr) => {
        fn visit_bool<E: de::Error>(self, _: bool) -> Result<Self::Value, E> {
            Ok($value)
        }

        fn visit_i64<E: de::Error>(self, _: i64) -> Result<Self::Value, E> {
            Ok($value)
        }

        fn visit_u64<E: de::Error>(self, _: u64) -> Result<Self::Value, E> {
            Ok($value)
        }

        fn visit_f64<E: de::Error>(self, _: f64) -> Result<Self::Value, E> {
            Ok($value)
        }

        fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
            Ok($value)
        }

        fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
            Ok($value)
        }
    };
}

struct Text(Option<String>);

impl<'de> Deserialize<'de> for Text {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(TextVisitor)
    }
}

struct TextVisitor;

impl<'de> Visitor<'de> for TextVisitor {
    type Value = Text;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a string")
    }

    ignoring_scalars!(Text(None));

    fn visit_str<E: de::Error>(self, text: &str) -> Result<Text, E> {
        Ok(Text(Some(text.to_owned())))
    }

    fn visit_string<E: de::Error>(self, text: String) -> Result<Text, E> {
        Ok(Text(Some(text)))
    }

    fn visit_map<M: MapAccess<'de>>(self, map: M) -> Result<Text, M::Error> {
        drained_map(map).map(|()| Text(None))
    }

    fn visit_seq<S: SeqAccess<'de>>(self, seq: S) -> Result<Text, S::Error> {
        drained_seq(seq).map(|()| Text(None))
    }
}

struct Texts(Vec<String>);

impl<'de> Deserialize<'de> for Texts {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(TextsVisitor)
    }
}

struct TextsVisitor;

impl<'de> Visitor<'de> for TextsVisitor {
    type Value = Texts;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a string or a list of strings")
    }

    ignoring_scalars!(Texts(Vec::new()));

    fn visit_str<E: de::Error>(self, text: &str) -> Result<Texts, E> {
        Ok(Texts(vec![text.to_owned()]))
    }

    fn visit_seq<S: SeqAccess<'de>>(self, mut seq: S) -> Result<Texts, S::Error> {
        let mut listed = Vec::new();
        while let Some(Text(text)) = seq.next_element::<Text>()? {
            listed.extend(text);
        }
        Ok(Texts(listed))
    }

    fn visit_map<M: MapAccess<'de>>(self, map: M) -> Result<Texts, M::Error> {
        drained_map(map).map(|()| Texts(Vec::new()))
    }
}

struct Licenses(Vec<License>);

impl<'de> Deserialize<'de> for Licenses {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(LicensesVisitor)
    }
}

struct LicensesVisitor;

impl<'de> Visitor<'de> for LicensesVisitor {
    type Value = Licenses;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a licence or a list of licences")
    }

    ignoring_scalars!(Licenses(Vec::new()));

    fn visit_str<E: de::Error>(self, name: &str) -> Result<Licenses, E> {
        Ok(Licenses(vec![named(name)]))
    }

    fn visit_map<M: MapAccess<'de>>(self, map: M) -> Result<Licenses, M::Error> {
        let licence = RawLicense::deserialize(MapAccessDeserializer::new(map))?;
        Ok(Licenses(vec![licence.into_license()]))
    }

    fn visit_seq<S: SeqAccess<'de>>(self, mut seq: S) -> Result<Licenses, S::Error> {
        let mut listed = Vec::new();
        while let Some(Licenses(one)) = seq.next_element::<Licenses>()? {
            listed.extend(one);
        }
        Ok(Licenses(listed))
    }
}

struct Keys(Vec<String>);

impl<'de> Deserialize<'de> for Keys {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_map(KeysVisitor)
    }
}

struct KeysVisitor;

impl<'de> Visitor<'de> for KeysVisitor {
    type Value = Keys;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("outputs by name")
    }

    fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Keys, M::Error> {
        let mut keys = Vec::new();
        while let Some((key, IgnoredAny)) = map.next_entry::<String, IgnoredAny>()? {
            keys.push(key);
        }
        Ok(Keys(keys))
    }
}
