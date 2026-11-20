use crate::name::language::{macintosh_language, match_score, windows_language};
use crate::name::mac_roman;
use crate::name::name_id::NameId;
use crate::parse::bytes::Bytes;
use crate::sfnt::font::Font;
use crate::sfnt::table_id::TableId;

const RECORD_SIZE: usize = 12;
const UNICODE: u16 = 0;
const MACINTOSH: u16 = 1;
const WINDOWS: u16 = 3;
const LANGUAGE_TAG_BASE: u16 = 0x8000;
const DEFAULT_LANGUAGE: &str = "en-US";

#[derive(Clone, Copy, Debug)]
pub struct Name<'a> {
    records: &'a [u8],
    storage: &'a [u8],
    language_tags: &'a [u8],
}

#[derive(Clone, Copy, Debug)]
pub struct NameRecord<'a> {
    pub platform_id: u16,
    pub encoding_id: u16,
    pub language_id: u16,
    pub name_id: NameId,
    bytes: &'a [u8],
    language_tag: Option<&'a [u8]>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NameLanguage<'a> {
    Known(&'static str),
    Tag(&'a [u8]),
    Unknown,
}

impl<'a> Name<'a> {
    pub fn new(data: &'a [u8]) -> Option<Self> {
        let format = data.u16_at(0)?;
        let count = usize::from(data.u16_at(2)?);
        let storage = data.tail(usize::from(data.u16_at(4)?)).unwrap_or_default();
        let records = data.range(6, count * RECORD_SIZE)?;
        let language_tags = if format == 1 {
            let tags_at = 6 + count * RECORD_SIZE;
            let tag_count = usize::from(data.u16_at(tags_at).unwrap_or(0));
            data.range(tags_at + 2, tag_count * 4).unwrap_or_default()
        } else {
            &[]
        };
        Some(Self {
            records,
            storage,
            language_tags,
        })
    }

    pub fn records(&self) -> impl Iterator<Item = NameRecord<'a>> + use<'a> {
        let name = *self;
        (0..self.records.len() / RECORD_SIZE).filter_map(move |index| name.record(index))
    }

    fn record(&self, index: usize) -> Option<NameRecord<'a>> {
        let record = self.records.range(index * RECORD_SIZE, RECORD_SIZE)?;
        let language_id = record.u16_at(4)?;
        let length = usize::from(record.u16_at(8)?);
        let offset = usize::from(record.u16_at(10)?);
        let language_tag = language_id
            .checked_sub(LANGUAGE_TAG_BASE)
            .and_then(|index| self.language_tag(usize::from(index)));
        Some(NameRecord {
            platform_id: record.u16_at(0)?,
            encoding_id: record.u16_at(2)?,
            language_id,
            name_id: NameId(record.u16_at(6)?),
            bytes: self.storage.range(offset, length)?,
            language_tag,
        })
    }

    fn language_tag(&self, index: usize) -> Option<&'a [u8]> {
        let length = usize::from(self.language_tags.u16_at(index * 4)?);
        let offset = usize::from(self.language_tags.u16_at(index * 4 + 2)?);
        self.storage.range(offset, length)
    }

    pub fn get(&self, name_id: NameId) -> Option<String> {
        self.localized(name_id, DEFAULT_LANGUAGE)
    }

    pub fn localized(&self, name_id: NameId, language: &str) -> Option<String> {
        self.best_record(name_id, language)
            .map(|record| record.to_string())
    }

    pub fn best_record(&self, name_id: NameId, language: &str) -> Option<NameRecord<'a>> {
        let mut best: Option<(u8, NameRecord<'a>)> = None;
        for record in self.records() {
            if record.name_id != name_id || !record.is_decodable() {
                continue;
            }
            let language_score = record.language_score(language);
            let english_score = record.language_score(DEFAULT_LANGUAGE);
            let platform_score = match record.platform_id {
                WINDOWS => 2,
                UNICODE => 1,
                _ => 0,
            };
            let score = language_score * 16 + english_score.min(3) * 4 + platform_score;
            if best.is_none_or(|(best_score, _)| score > best_score) {
                best = Some((score, record));
            }
        }
        best.map(|(_, record)| record)
    }
}

impl<'a> NameRecord<'a> {
    #[inline]
    pub fn bytes(&self) -> &'a [u8] {
        self.bytes
    }

    pub fn language(&self) -> NameLanguage<'a> {
        if let Some(tag) = self.language_tag {
            return NameLanguage::Tag(tag);
        }
        let known = match self.platform_id {
            WINDOWS => windows_language(self.language_id),
            MACINTOSH => macintosh_language(self.language_id),
            _ => None,
        };
        known.map_or(NameLanguage::Unknown, NameLanguage::Known)
    }

    fn language_score(&self, requested: &str) -> u8 {
        match self.language() {
            NameLanguage::Known(tag) => match_score(tag, requested),
            NameLanguage::Tag(utf16) => {
                let mut tag = [0u8; 32];
                let mut len = 0;
                for &[high, low] in utf16.as_chunks::<2>().0 {
                    if high != 0 || !low.is_ascii() || len == tag.len() {
                        return 0;
                    }
                    tag[len] = low;
                    len += 1;
                }
                std::str::from_utf8(&tag[..len]).map_or(0, |tag| match_score(tag, requested))
            }
            NameLanguage::Unknown => 0,
        }
    }

    pub fn is_decodable(&self) -> bool {
        self.encoding().is_some()
    }

    fn encoding(&self) -> Option<Encoding> {
        match (self.platform_id, self.encoding_id) {
            (UNICODE, _) | (WINDOWS, 0 | 1 | 10) => Some(Encoding::Utf16),
            (MACINTOSH, 0) => Some(Encoding::MacRoman),
            _ => None,
        }
    }

    pub fn chars(&self) -> impl Iterator<Item = char> + use<'a> {
        let bytes = self.bytes;
        let encoding = self.encoding();
        let utf16 = (encoding == Some(Encoding::Utf16)).then(|| {
            char::decode_utf16(
                bytes
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|unit| u16::from_be_bytes(*unit)),
            )
            .map(|c| c.unwrap_or(char::REPLACEMENT_CHARACTER))
        });
        let mac = (encoding == Some(Encoding::MacRoman))
            .then(|| bytes.iter().map(|&byte| mac_roman::decode(byte)));
        utf16.into_iter().flatten().chain(mac.into_iter().flatten())
    }
}

impl std::fmt::Display for NameRecord<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.chars()
            .try_for_each(|c| std::fmt::Write::write_char(f, c))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Encoding {
    Utf16,
    MacRoman,
}

impl<'a> Font<'a> {
    pub fn name(&self) -> Option<Name<'a>> {
        Name::new(self.table_data(TableId::Name)?)
    }
}
