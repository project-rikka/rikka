use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::{host, web};

const EMOJI_VERSION: &str = "16.0";
const CLDR_VERSION: &str = "47.0.0";

const EMOJI_DATA: &str = "crates/foundation/rikka-emoji/data";
const EMOJI_TABLE: &str = "emoji.txt";

const TONES: [char; 5] = [
    '\u{1f3fb}',
    '\u{1f3fc}',
    '\u{1f3fd}',
    '\u{1f3fe}',
    '\u{1f3ff}',
];
const PRESENTATION: char = '\u{fe0f}';

const QUALIFIED: &str = "fully-qualified";

const CATALOGUES: &[(&str, &str)] = &[("en", "en"), ("ja", "ja"), ("zh-Hans", "zh")];

const GROUPS: &[(&str, &str)] = &[
    ("Smileys & Emotion", "smileys"),
    ("People & Body", "people"),
    ("Animals & Nature", "nature"),
    ("Food & Drink", "food"),
    ("Travel & Places", "travel"),
    ("Activities", "activities"),
    ("Objects", "objects"),
    ("Symbols", "symbols"),
    ("Flags", "flags"),
];

const COMPONENTS: &str = "Component";

pub(crate) fn emoji(workspace: &Path) -> Result<()> {
    let directory = workspace.join(EMOJI_DATA);
    let emoji = read_emoji()?;

    let table_path = directory.join(EMOJI_TABLE);
    host::write(&table_path, &table(&emoji)?)?;
    println!("wrote {} emoji to {}", emoji.len(), table_path.display());

    for (locale, cldr) in CATALOGUES {
        let annotations = read_annotations(cldr)?;
        let path = directory.join(format!("{locale}.txt"));
        let catalogue = catalogue(locale, &emoji, &annotations)?;
        host::write(&path, &catalogue.document)?;
        println!(
            "wrote {locale} names to {} ({} fall back to English)",
            path.display(),
            catalogue.unnamed
        );
    }
    Ok(())
}

fn cldr<T: DeserializeOwned>(path: &str) -> Result<T> {
    web::json(&format!(
        "https://raw.githubusercontent.com/unicode-org/cldr-json/{CLDR_VERSION}/cldr-json/{path}"
    ))
}

struct Emoji {
    text: String,
    group: &'static str,
    name: String,
    tones: [Option<String>; 5],
}

impl Emoji {
    fn toned(&self) -> Result<Option<Vec<&str>>> {
        let found = self.tones.iter().flatten().count();
        match found {
            0 => Ok(None),
            5 => Ok(Some(
                self.tones
                    .iter()
                    .map(|tone| tone.as_deref().unwrap_or_default())
                    .collect(),
            )),
            found => bail!("`{}` has {found} skin tones, expected 0 or 5", self.text),
        }
    }
}

fn table(emoji: &[Emoji]) -> Result<String> {
    let mut table = format!("# Unicode Emoji {EMOJI_VERSION}\n");
    for one in emoji {
        write!(table, "{}\t{}", one.text, one.group)?;
        if let Some(tones) = one.toned()? {
            write!(table, "\t{}", tones.join(" "))?;
        }
        table.push('\n');
    }
    Ok(table)
}

struct Catalogue {
    document: String,
    unnamed: usize,
}

fn catalogue(locale: &str, emoji: &[Emoji], annotations: &Annotations) -> Result<Catalogue> {
    let mut document = format!("# CLDR {CLDR_VERSION} {locale}\n");
    let mut unnamed = 0;

    for one in emoji {
        let annotation = annotations.get(&stripped(&one.text));
        let name = annotation
            .and_then(|annotation| annotation.tts.first())
            .unwrap_or(&one.name);
        if annotation.is_none() {
            unnamed += 1;
        }
        write!(document, "{}\t{name}", one.text)?;
        for keyword in annotation.iter().flat_map(|annotation| &annotation.default) {
            write!(document, "\t{keyword}")?;
        }
        document.push('\n');
    }
    Ok(Catalogue { document, unnamed })
}

fn read_emoji() -> Result<Vec<Emoji>> {
    let url = format!("https://unicode.org/Public/emoji/{EMOJI_VERSION}/emoji-test.txt");
    let listing = web::text(&url)?;

    let mut emoji: Vec<Emoji> = Vec::new();
    let mut found: HashMap<String, usize> = HashMap::new();
    let mut group = None;

    for line in listing.lines() {
        if let Some(named) = line.strip_prefix("# group: ") {
            group = group_of(named.trim())?;
            continue;
        }
        let Some(listed) = listed(line) else {
            continue;
        };
        let Some(group) = group else {
            continue;
        };

        let text = spelled(listed.codepoints)?;
        match toning(&text) {
            Toning::Plain => {
                found.insert(stripped(&text), emoji.len());
                emoji.push(Emoji {
                    text,
                    group,
                    name: listed.name.to_owned(),
                    tones: [const { None }; 5],
                });
            }
            Toning::One(tone) => {
                let base = stripped(&text.replace(TONES, ""));
                if let Some(index) = found.get(&base) {
                    emoji[*index].tones[tone] = Some(text);
                }
            }
            Toning::Mixed => {}
        }
    }

    match emoji.is_empty() {
        true => bail!("{url} lists no emoji; has the format changed?"),
        false => Ok(emoji),
    }
}

struct Listed<'a> {
    codepoints: &'a str,
    name: &'a str,
}

fn listed(line: &str) -> Option<Listed<'_>> {
    let (data, comment) = line.split_once('#')?;
    let (codepoints, status) = data.split_once(';')?;
    if status.trim() != QUALIFIED {
        return None;
    }
    let mut words = comment.split_whitespace().skip(1);
    let version = words.next()?;
    if !version.starts_with('E') {
        return None;
    }
    let name = comment.split_once(version)?.1.trim();
    Some(Listed {
        codepoints: codepoints.trim(),
        name,
    })
}

fn spelled(codepoints: &str) -> Result<String> {
    codepoints
        .split_whitespace()
        .map(|codepoint| {
            u32::from_str_radix(codepoint, 16)
                .ok()
                .and_then(char::from_u32)
                .with_context(|| format!("invalid code point `{codepoint}`"))
        })
        .collect()
}

fn group_of(named: &str) -> Result<Option<&'static str>> {
    if named == COMPONENTS {
        return Ok(None);
    }
    GROUPS
        .iter()
        .find(|(listed, _)| *listed == named)
        .map(|(_, slug)| Some(*slug))
        .with_context(|| format!("unknown emoji group `{named}`"))
}

enum Toning {
    Plain,
    One(usize),
    Mixed,
}

fn toning(text: &str) -> Toning {
    let mut tones = text
        .chars()
        .filter_map(|character| TONES.iter().position(|tone| *tone == character));
    let Some(first) = tones.next() else {
        return Toning::Plain;
    };
    match tones.all(|tone| tone == first) {
        true => Toning::One(first),
        false => Toning::Mixed,
    }
}

fn stripped(text: &str) -> String {
    text.replace(PRESENTATION, "")
}

type Annotations = HashMap<String, Annotation>;

#[derive(Deserialize)]
struct Annotation {
    #[serde(default)]
    default: Vec<String>,
    #[serde(default)]
    tts: Vec<String>,
}

#[derive(Deserialize)]
struct AnnotationDocument {
    #[serde(rename = "annotations", alias = "annotationsDerived")]
    body: AnnotationBody,
}

#[derive(Deserialize)]
struct AnnotationBody {
    annotations: Annotations,
}

fn read_annotations(locale: &str) -> Result<Annotations> {
    let derived: AnnotationDocument = cldr(&format!(
        "cldr-annotations-derived-full/annotationsDerived/{locale}/annotations.json"
    ))?;
    let named: AnnotationDocument = cldr(&format!(
        "cldr-annotations-full/annotations/{locale}/annotations.json"
    ))?;
    let mut annotations = derived.body.annotations;
    annotations.extend(named.body.annotations);
    Ok(annotations)
}
