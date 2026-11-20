use rikka_text_font::{Font, FontFile, Location, Tag};
use rikka_text_shape::{
    Buffer, BufferFlags, Direction, Features, Language, SegmentProperties, ShapeFace, ShapePlan,
};
use rikka_text_unicode::Script;
use std::str::FromStr;

use crate::corpus::Case;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShapedGlyph {
    pub glyph: u32,
    pub cluster: u32,
    pub x_advance: i32,
    pub y_advance: i32,
    pub x_offset: i32,
    pub y_offset: i32,
    pub unsafe_to_break: bool,
    pub unsafe_to_concat: bool,
}

#[derive(Debug)]
pub struct Prepared {
    data: &'static [u8],
    face_index: u32,
    pub text: String,
    pub properties: SegmentProperties,
    pub features: Features,
    pub flags: BufferFlags,
    location: Location,
}

impl Prepared {
    pub fn new(case: &Case, text: &str, data: &'static [u8], produce_concat: bool) -> Option<Self> {
        let file = FontFile::new(data).ok()?;
        let font = file.face(case.face).ok()?;
        let location = font.location(
            case.variations
                .iter()
                .map(|(tag, value)| (Tag::new(tag), *value)),
        );
        let mut probe = Buffer::new();
        probe.push_str(text, 0);
        let mut properties = match case.script.and_then(Script::from_code) {
            Some(script) => SegmentProperties::new(script),
            None => probe.guess_properties(),
        };
        if let Some(rtl) = case.rtl {
            properties.direction = if rtl {
                Direction::RightToLeft
            } else {
                Direction::LeftToRight
            };
        }
        properties.language = case
            .language
            .and_then(|language| Language::from_str(language).ok());
        let mut flags = BufferFlags::empty();
        if case.bot_eot {
            flags |= BufferFlags::BEGINNING_OF_TEXT | BufferFlags::END_OF_TEXT;
        }
        if produce_concat {
            flags |= BufferFlags::PRODUCE_UNSAFE_TO_CONCAT;
        }
        let features = Features::from_str(case.features).ok()?;
        Some(Self {
            face_index: case.face,
            text: text.to_owned(),
            properties,
            features,
            flags,
            location,
            data,
        })
    }

    pub fn font(&self) -> Option<Font<'static>> {
        FontFile::new(self.data).ok()?.face(self.face_index).ok()
    }

    pub fn data(&self) -> &'static [u8] {
        self.data
    }

    pub fn face_index(&self) -> u32 {
        self.face_index
    }

    pub fn location(&self) -> &Location {
        &self.location
    }
}

#[derive(Debug)]
pub struct RikkaEngine<'a> {
    prepared: &'a Prepared,
    plan: ShapePlan,
}

impl<'a> RikkaEngine<'a> {
    pub fn new(prepared: &'a Prepared) -> Option<Self> {
        let face = ShapeFace::new(prepared.font()?, prepared.location.clone());
        let plan = face.plan(&prepared.properties, &prepared.features);
        Some(Self { prepared, plan })
    }

    pub fn buffer(&self) -> Buffer {
        let mut buffer = Buffer::new();
        buffer.set_flags(self.prepared.flags);
        buffer
    }

    pub fn shape(&self, buffer: &mut Buffer, text: &str) -> usize {
        buffer.clear();
        buffer.push_str(text, 0);
        self.plan.shape(buffer);
        buffer.len()
    }

    pub fn shape_corpus_text(&self) -> Vec<ShapedGlyph> {
        let mut buffer = self.buffer();
        buffer.push_str(&self.prepared.text, 0);
        self.plan.shape(&mut buffer);
        buffer
            .glyph_infos()
            .iter()
            .zip(buffer.glyph_positions())
            .map(|(info, position)| ShapedGlyph {
                glyph: u32::from(info.glyph().0),
                cluster: info.cluster(),
                x_advance: position.x_advance,
                y_advance: position.y_advance,
                x_offset: position.x_offset,
                y_offset: position.y_offset,
                unsafe_to_break: info.unsafe_to_break(),
                unsafe_to_concat: info.unsafe_to_concat(),
            })
            .collect()
    }
}

pub struct HarfrustEngine<'a> {
    shaper_data: harfrust::ShaperData,
    instance: harfrust::ShaperInstance,
    font: harfrust::FontRef<'a>,
    features: Vec<harfrust::Feature>,
    direction: harfrust::Direction,
    script: Option<harfrust::Script>,
    language: Option<harfrust::Language>,
    flags: harfrust::BufferFlags,
    text: String,
}

impl<'a> HarfrustEngine<'a> {
    pub fn new(prepared: &'a Prepared) -> Option<Self> {
        let font = harfrust::FontRef::from_index(prepared.data(), prepared.face_index()).ok()?;
        let shaper_data = harfrust::ShaperData::new(&font);
        let coords = prepared
            .location()
            .coords()
            .iter()
            .map(|coord| harfrust::NormalizedCoord::from_bits(coord.0));
        let instance = harfrust::ShaperInstance::from_coords(&font, coords);
        let features = prepared
            .features
            .iter()
            .map(|feature| harfrust::Feature {
                tag: harfrust::Tag::new(&feature.tag.to_bytes()),
                value: feature.value,
                start: feature.start,
                end: feature.end,
            })
            .collect();
        let properties = prepared.properties;
        let direction = match properties.direction {
            Direction::LeftToRight => harfrust::Direction::LeftToRight,
            Direction::RightToLeft => harfrust::Direction::RightToLeft,
        };
        let code = properties.script.code().as_bytes();
        let script = <[u8; 4]>::try_from(code)
            .ok()
            .and_then(|code| harfrust::Script::from_iso15924_tag(harfrust::Tag::new(&code)));
        let language = properties
            .language
            .and_then(|language| harfrust::Language::from_str(language.as_str()).ok());
        let mut flags = harfrust::BufferFlags::empty();
        if prepared.flags.contains(BufferFlags::BEGINNING_OF_TEXT) {
            flags |= harfrust::BufferFlags::BEGINNING_OF_TEXT | harfrust::BufferFlags::END_OF_TEXT;
        }
        if prepared
            .flags
            .contains(BufferFlags::PRODUCE_UNSAFE_TO_CONCAT)
        {
            flags |= harfrust::BufferFlags::PRODUCE_UNSAFE_TO_CONCAT;
        }
        Some(Self {
            shaper_data,
            instance,
            font,
            features,
            direction,
            script,
            language,
            flags,
            text: prepared.text.clone(),
        })
    }

    pub fn shaper(&self) -> harfrust::Shaper<'_> {
        self.shaper_data
            .shaper(&self.font)
            .instance(Some(&self.instance))
            .build()
    }

    pub fn plan(&self, shaper: &harfrust::Shaper<'_>) -> harfrust::ShapePlan {
        harfrust::ShapePlan::new(
            shaper,
            self.direction,
            self.script,
            self.language.as_ref(),
            &self.features,
        )
    }

    pub fn shape(
        &self,
        shaper: &harfrust::Shaper<'_>,
        plan: &harfrust::ShapePlan,
        text: &str,
        mut buffer: harfrust::UnicodeBuffer,
    ) -> harfrust::GlyphBuffer {
        buffer.push_str(text);
        buffer.set_direction(self.direction);
        if let Some(script) = self.script {
            buffer.set_script(script);
        }
        if let Some(language) = self.language.clone() {
            buffer.set_language(language);
        }
        buffer.set_flags(self.flags);
        shaper.shape_with_plan(plan, buffer, &self.features)
    }

    pub fn shape_corpus_text(&self) -> Vec<ShapedGlyph> {
        let shaper = self.shaper();
        let plan = self.plan(&shaper);
        let output = self.shape(&shaper, &plan, &self.text, harfrust::UnicodeBuffer::new());
        output
            .glyph_infos()
            .iter()
            .zip(output.glyph_positions())
            .map(|(info, position)| ShapedGlyph {
                glyph: info.glyph_id,
                cluster: info.cluster,
                x_advance: position.x_advance,
                y_advance: position.y_advance,
                x_offset: position.x_offset,
                y_offset: position.y_offset,
                unsafe_to_break: info.unsafe_to_break(),
                unsafe_to_concat: info.unsafe_to_concat(),
            })
            .collect()
    }
}

impl std::fmt::Debug for HarfrustEngine<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HarfrustEngine")
            .field("direction", &self.direction)
            .field("script", &self.script)
            .field("text", &self.text)
            .finish_non_exhaustive()
    }
}
