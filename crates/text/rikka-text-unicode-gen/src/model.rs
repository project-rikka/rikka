use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::aliases::{Aliases, ValueSet};
use crate::enumeration::Column;
use crate::error::{Error, Result};
use crate::source::Directory;
use crate::ucd::{CODEPOINT_COUNT, File, Record, parse_codepoint};

const HANGUL_SYLLABLES: std::ops::RangeInclusive<u32> = 0xAC00..=0xD7A3;
const HANGUL_VOWELS: std::ops::RangeInclusive<u32> = 0x1161..=0x1175;
const HANGUL_TRAILS: std::ops::RangeInclusive<u32> = 0x11A8..=0x11C2;

#[derive(Debug)]
pub(crate) struct Model {
    pub(crate) general_category: Column,
    pub(crate) script: Column,
    pub(crate) bidi_class: Column,
    pub(crate) line_break: Column,
    pub(crate) grapheme_cluster_break: Column,
    pub(crate) word_break: Column,
    pub(crate) east_asian_width: Column,
    pub(crate) joining_type: Column,
    pub(crate) joining_group: Column,
    pub(crate) indic_syllabic_category: Column,
    pub(crate) indic_positional_category: Column,
    pub(crate) indic_conjunct_break: Column,
    pub(crate) combining_class: Vec<u8>,
    pub(crate) bracket_type: Vec<u8>,
    pub(crate) flags: Flags,
    pub(crate) script_extensions: Vec<u16>,
    pub(crate) script_extension_sets: Vec<Vec<u8>>,
    pub(crate) decompositions: BTreeMap<u32, (u32, Option<u32>)>,
    pub(crate) compositions: BTreeMap<u32, Vec<(u32, u32)>>,
    pub(crate) compatibility: BTreeMap<u32, Vec<u32>>,
    pub(crate) mirrors: Vec<(u32, u32)>,
    pub(crate) brackets: Vec<(u32, u32)>,
}

#[derive(Debug)]
pub(crate) struct Flags {
    pub(crate) extended_pictographic: Vec<bool>,
    pub(crate) emoji: Vec<bool>,
    pub(crate) emoji_presentation: Vec<bool>,
    pub(crate) emoji_modifier: Vec<bool>,
    pub(crate) emoji_modifier_base: Vec<bool>,
    pub(crate) emoji_component: Vec<bool>,
    pub(crate) default_ignorable: Vec<bool>,
    pub(crate) bidi_mirrored: Vec<bool>,
    pub(crate) decomposes: Vec<bool>,
    pub(crate) nfc_no: Vec<bool>,
    pub(crate) nfc_maybe: Vec<bool>,
}

struct Brackets {
    pairs: Vec<(u32, u32)>,
    kinds: Vec<u8>,
}

struct Loader<'a> {
    source: &'a Directory,
    aliases: Aliases,
}

impl Model {
    pub(crate) fn load(source: &Directory) -> Result<Self> {
        let aliases = Aliases::parse(&source.read("PropertyValueAliases.txt")?)?;
        let loader = Loader { source, aliases };
        let unicode_data = source.read("UnicodeData.txt")?;
        let unicode_data = UnicodeData::parse(
            &File::new("UnicodeData.txt", &unicode_data),
            loader.aliases.values("gc")?,
        )?;

        let general_category = Column::compact(
            "GeneralCategory",
            &unicode_data.general_category_set,
            &unicode_data.general_category,
        )?;
        let script = loader.enumerated("Script", "sc", "Scripts.txt", None)?;
        let bidi_class =
            loader.enumerated("BidiClass", "bc", "extracted/DerivedBidiClass.txt", None)?;
        let line_break = loader.enumerated("LineBreakClass", "lb", "LineBreak.txt", None)?;
        let grapheme_cluster_break = loader.enumerated(
            "GraphemeClusterBreak",
            "GCB",
            "auxiliary/GraphemeBreakProperty.txt",
            None,
        )?;
        let word_break =
            loader.enumerated("WordBreak", "WB", "auxiliary/WordBreakProperty.txt", None)?;
        let east_asian_width =
            loader.enumerated("EastAsianWidth", "ea", "EastAsianWidth.txt", None)?;
        let joining_type = loader.enumerated(
            "JoiningType",
            "jt",
            "extracted/DerivedJoiningType.txt",
            None,
        )?;
        let joining_group = loader.enumerated(
            "JoiningGroup",
            "jg",
            "extracted/DerivedJoiningGroup.txt",
            None,
        )?;
        let indic_syllabic_category = loader.enumerated(
            "IndicSyllabicCategory",
            "InSC",
            "IndicSyllabicCategory.txt",
            None,
        )?;
        let indic_positional_category = loader.enumerated(
            "IndicPositionalCategory",
            "InPC",
            "IndicPositionalCategory.txt",
            None,
        )?;
        let indic_conjunct_break = loader.enumerated(
            "IndicConjunctBreak",
            "InCB",
            "DerivedCoreProperties.txt",
            Some("InCB"),
        )?;

        let emoji = source.read("emoji/emoji-data.txt")?;
        let emoji = File::new("emoji/emoji-data.txt", &emoji);
        let core = source.read("DerivedCoreProperties.txt")?;
        let core = File::new("DerivedCoreProperties.txt", &core);
        let exclusions = source.read("CompositionExclusions.txt")?;
        let exclusions = File::new("CompositionExclusions.txt", &exclusions);
        let normalization = Normalization::derive(&unicode_data, &exclusions)?;

        let flags = Flags {
            extended_pictographic: binary(&emoji, "Extended_Pictographic")?,
            emoji: binary(&emoji, "Emoji")?,
            emoji_presentation: binary(&emoji, "Emoji_Presentation")?,
            emoji_modifier: binary(&emoji, "Emoji_Modifier")?,
            emoji_modifier_base: binary(&emoji, "Emoji_Modifier_Base")?,
            emoji_component: binary(&emoji, "Emoji_Component")?,
            default_ignorable: binary(&core, "Default_Ignorable_Code_Point")?,
            bidi_mirrored: unicode_data.bidi_mirrored.clone(),
            decomposes: normalization.decomposes,
            nfc_no: normalization.nfc_no,
            nfc_maybe: normalization.nfc_maybe,
        };

        let (script_extensions, script_extension_sets) = loader.script_extensions(&script)?;
        let mirrors = loader.mirrors()?;
        let Brackets {
            pairs: brackets,
            kinds: bracket_type,
        } = loader.brackets()?;

        Ok(Self {
            general_category,
            script,
            bidi_class,
            line_break,
            grapheme_cluster_break,
            word_break,
            east_asian_width,
            joining_type,
            joining_group,
            indic_syllabic_category,
            indic_positional_category,
            indic_conjunct_break,
            combining_class: unicode_data.combining_class,
            bracket_type,
            flags,
            script_extensions,
            script_extension_sets,
            decompositions: unicode_data.decompositions,
            compositions: normalization.compositions,
            compatibility: normalization.compatibility,
            mirrors,
            brackets,
        })
    }
}

impl Loader<'_> {
    fn enumerated(
        &self,
        name: &'static str,
        property: &str,
        file_name: &str,
        selector: Option<&str>,
    ) -> Result<Column> {
        let set = self.aliases.values(property)?;
        let text = self.source.read(file_name)?;
        let file = File::new(file_name, &text);
        let mut raw = vec![u16::MAX; CODEPOINT_COUNT];
        file.fill(&mut raw, |record| {
            let value = match selector {
                Some(selector) if record.field(0) != selector => return Ok(None),
                Some(_) => record.field(1),
                None => record.field(0),
            };
            lookup(&set, &file, record, value).map(Some)
        })?;
        if let Some(missing) = raw.iter().position(|&value| value == u16::MAX) {
            return Err(Error::Data(format!(
                "{file_name} assigns no {property} to U+{missing:04X}"
            )));
        }
        Column::compact(name, &set, &raw)
    }

    fn script_extensions(&self, script: &Column) -> Result<(Vec<u16>, Vec<Vec<u8>>)> {
        let set = self.aliases.values("sc")?;
        let text = self.source.read("ScriptExtensions.txt")?;
        let file = File::new("ScriptExtensions.txt", &text);
        let mut sets: Vec<Vec<u8>> = Vec::new();
        let mut positions: HashMap<Vec<u8>, u16> = HashMap::new();
        let mut column = vec![0u16; CODEPOINT_COUNT];
        for record in file.records()? {
            let mut scripts = BTreeSet::new();
            for code in record.field(0).split_whitespace() {
                let value = set.lookup(&file, record.line, code)?;
                scripts.insert(script.enumeration.position(set.short(value))?);
            }
            let scripts: Vec<u8> = scripts.into_iter().collect();
            let position = match positions.get(&scripts) {
                Some(&position) => position,
                None => {
                    sets.push(scripts.clone());
                    let position = u16::try_from(sets.len())
                        .map_err(|_| Error::Data("too many script extension sets".into()))?;
                    positions.insert(scripts, position);
                    position
                }
            };
            column[record.first as usize..=record.last as usize].fill(position);
        }
        Ok((column, sets))
    }

    fn mirrors(&self) -> Result<Vec<(u32, u32)>> {
        let text = self.source.read("BidiMirroring.txt")?;
        let file = File::new("BidiMirroring.txt", &text);
        file.records()?
            .iter()
            .map(|record| Ok((record.first, codepoint(&file, record, 0)?)))
            .collect()
    }

    fn brackets(&self) -> Result<Brackets> {
        let text = self.source.read("BidiBrackets.txt")?;
        let file = File::new("BidiBrackets.txt", &text);
        let mut pairs = Vec::new();
        let mut kinds = vec![0u8; CODEPOINT_COUNT];
        for record in file.records()? {
            kinds[record.first as usize] = match record.field(1) {
                "o" => 1,
                "c" => 2,
                other => {
                    return Err(file.error(record.line, format!("unknown bracket type `{other}`")));
                }
            };
            pairs.push((record.first, codepoint(&file, &record, 0)?));
        }
        Ok(Brackets { pairs, kinds })
    }
}

fn lookup(set: &ValueSet, file: &File<'_>, record: &Record<'_>, value: &str) -> Result<u16> {
    let index = set.lookup(file, record.line, value)?;
    Ok(index as u16)
}

fn codepoint(file: &File<'_>, record: &Record<'_>, field: usize) -> Result<u32> {
    parse_codepoint(record.field(field)).ok_or_else(|| {
        file.error(
            record.line,
            format!("invalid code point `{}`", record.field(field)),
        )
    })
}

fn binary(file: &File<'_>, property: &str) -> Result<Vec<bool>> {
    let mut column = vec![false; CODEPOINT_COUNT];
    file.fill(&mut column, |record| {
        Ok((record.field(0) == property && record.fields.len() == 1).then_some(true))
    })?;
    if column.iter().all(|&value| !value) {
        return Err(Error::Data(format!("no code point has {property}")));
    }
    Ok(column)
}

struct UnicodeData {
    general_category_set: ValueSet,
    general_category: Vec<u16>,
    combining_class: Vec<u8>,
    bidi_mirrored: Vec<bool>,
    decompositions: BTreeMap<u32, (u32, Option<u32>)>,
    tagged_decompositions: BTreeMap<u32, Vec<u32>>,
}

impl UnicodeData {
    fn parse(file: &File<'_>, general_category_set: ValueSet) -> Result<Self> {
        let unassigned = general_category_set.lookup(file, 0, "Cn")? as u16;
        let mut general_category = vec![unassigned; CODEPOINT_COUNT];
        let mut combining_class = vec![0u8; CODEPOINT_COUNT];
        let mut bidi_mirrored = vec![false; CODEPOINT_COUNT];
        let mut decompositions = BTreeMap::new();
        let mut tagged_decompositions = BTreeMap::new();
        let mut range_start = None;
        for record in file.records()? {
            let name = record.field(0);
            let first = match range_start.take() {
                Some(start) if name.ends_with(", Last>") => start,
                _ if name.ends_with(", First>") => {
                    range_start = Some(record.first);
                    continue;
                }
                _ => record.first,
            };
            let category = general_category_set.lookup(file, record.line, record.field(1))? as u16;
            let class: u8 = record
                .field(2)
                .parse()
                .map_err(|_| file.error(record.line, "invalid combining class"))?;
            let mirrored = record.field(8) == "Y";
            for codepoint in first..=record.first {
                general_category[codepoint as usize] = category;
                combining_class[codepoint as usize] = class;
                bidi_mirrored[codepoint as usize] = mirrored;
            }
            let decomposition = record.field(4);
            let (tagged, mapping) = match decomposition.strip_prefix('<') {
                Some(tagged) => {
                    let (_, mapping) = tagged
                        .split_once('>')
                        .ok_or_else(|| file.error(record.line, "unterminated decomposition tag"))?;
                    (true, mapping)
                }
                None => (false, decomposition),
            };
            let parts: Vec<u32> = mapping
                .split_whitespace()
                .map(|part| {
                    parse_codepoint(part)
                        .ok_or_else(|| file.error(record.line, "invalid decomposition"))
                })
                .collect::<Result<_>>()?;
            if tagged {
                tagged_decompositions.insert(record.first, parts);
            } else if !parts.is_empty() {
                let mapping = match parts.as_slice() {
                    [single] => (*single, None),
                    [first, second] => (*first, Some(*second)),
                    _ => return Err(file.error(record.line, "decomposition longer than two")),
                };
                decompositions.insert(record.first, mapping);
            }
        }
        Ok(Self {
            general_category_set,
            general_category,
            combining_class,
            bidi_mirrored,
            decompositions,
            tagged_decompositions,
        })
    }
}

struct Normalization {
    decomposes: Vec<bool>,
    nfc_no: Vec<bool>,
    nfc_maybe: Vec<bool>,
    compositions: BTreeMap<u32, Vec<(u32, u32)>>,
    compatibility: BTreeMap<u32, Vec<u32>>,
}

impl Normalization {
    fn derive(data: &UnicodeData, exclusions: &File<'_>) -> Result<Self> {
        let mut excluded: BTreeSet<u32> = exclusions
            .records()?
            .iter()
            .flat_map(|record| record.first..=record.last)
            .collect();
        for (&codepoint, &(first, second)) in &data.decompositions {
            let class = |codepoint: u32| data.combining_class[codepoint as usize];
            if second.is_none() || class(codepoint) != 0 || class(first) != 0 {
                excluded.insert(codepoint);
            }
        }
        let mut decomposes = vec![false; CODEPOINT_COUNT];
        let mut nfc_no = vec![false; CODEPOINT_COUNT];
        let mut nfc_maybe = vec![false; CODEPOINT_COUNT];
        let mut compositions: BTreeMap<u32, Vec<(u32, u32)>> = BTreeMap::new();
        for (&codepoint, &(first, second)) in &data.decompositions {
            decomposes[codepoint as usize] = true;
            if excluded.contains(&codepoint) {
                nfc_no[codepoint as usize] = true;
            } else if let Some(second) = second {
                nfc_maybe[second as usize] = true;
                compositions
                    .entry(first)
                    .or_default()
                    .push((second, codepoint));
            }
        }
        for codepoint in HANGUL_SYLLABLES {
            decomposes[codepoint as usize] = true;
        }
        for codepoint in HANGUL_VOWELS.chain(HANGUL_TRAILS) {
            nfc_maybe[codepoint as usize] = true;
        }
        let leading = |mut codepoint: u32| {
            while let Some(&(first, _)) = data.decompositions.get(&codepoint) {
                codepoint = first;
            }
            codepoint
        };
        let combines_backwards: Vec<u32> = data
            .decompositions
            .keys()
            .copied()
            .filter(|&codepoint| {
                !nfc_no[codepoint as usize] && nfc_maybe[leading(codepoint) as usize]
            })
            .collect();
        for codepoint in combines_backwards {
            nfc_maybe[codepoint as usize] = true;
        }
        for group in compositions.values_mut() {
            group.sort_unstable();
        }
        Ok(Self {
            decomposes,
            nfc_no,
            nfc_maybe,
            compositions,
            compatibility: compatibility(data)?,
        })
    }
}

fn compatibility(data: &UnicodeData) -> Result<BTreeMap<u32, Vec<u32>>> {
    let mut expansions = BTreeMap::new();
    let mapped = data
        .tagged_decompositions
        .keys()
        .chain(data.decompositions.keys());
    for &codepoint in mapped {
        let mut canonical = Vec::new();
        expand(data, codepoint, false, &mut canonical);
        let mut compatible = Vec::new();
        expand(data, codepoint, true, &mut compatible);
        if compatible == canonical {
            continue;
        }
        if compatible
            .iter()
            .any(|part| HANGUL_SYLLABLES.contains(part))
        {
            return Err(Error::Data(format!(
                "compatibility decomposition of U+{codepoint:04X} holds a Hangul syllable"
            )));
        }
        expansions.insert(codepoint, compatible);
    }
    Ok(expansions)
}

fn expand(data: &UnicodeData, codepoint: u32, tagged: bool, parts: &mut Vec<u32>) {
    let compatible = data
        .tagged_decompositions
        .get(&codepoint)
        .filter(|_| tagged);
    if let Some(mapping) = compatible {
        for &part in mapping {
            expand(data, part, tagged, parts);
        }
    } else if let Some(&(first, second)) = data.decompositions.get(&codepoint) {
        expand(data, first, tagged, parts);
        if let Some(second) = second {
            expand(data, second, tagged, parts);
        }
    } else {
        parts.push(codepoint);
    }
}
