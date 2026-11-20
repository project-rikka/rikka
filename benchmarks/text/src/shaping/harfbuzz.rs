use std::ffi::{c_char, c_int, c_uint, c_void};

use rikka_text_shape::{BufferFlags, Direction};

use crate::engines::{Prepared, ShapedGlyph};

enum Blob {}
enum Face {}
enum Font {}
enum HbBuffer {}
enum HbLanguage {}

#[repr(C)]
struct Feature {
    tag: u32,
    value: u32,
    start: c_uint,
    end: c_uint,
}

#[repr(C)]
struct GlyphInfo {
    codepoint: u32,
    mask: u32,
    cluster: u32,
    var1: u32,
    var2: u32,
}

#[repr(C)]
struct GlyphPosition {
    x_advance: i32,
    y_advance: i32,
    x_offset: i32,
    y_offset: i32,
    var: u32,
}

const MEMORY_MODE_READONLY: c_int = 1;
const DIRECTION_LTR: c_int = 4;
const DIRECTION_RTL: c_int = 5;
const FLAG_BOT: c_uint = 0x1;
const FLAG_EOT: c_uint = 0x2;
const FLAG_PRODUCE_UNSAFE_TO_CONCAT: c_uint = 0x40;
const GLYPH_FLAG_UNSAFE_TO_BREAK: c_uint = 0x1;
const GLYPH_FLAG_UNSAFE_TO_CONCAT: c_uint = 0x2;

unsafe extern "C" {
    fn hb_blob_create(
        data: *const c_char,
        length: c_uint,
        mode: c_int,
        user_data: *mut c_void,
        destroy: Option<unsafe extern "C" fn(*mut c_void)>,
    ) -> *mut Blob;
    fn hb_blob_destroy(blob: *mut Blob);
    fn hb_face_create(blob: *mut Blob, index: c_uint) -> *mut Face;
    fn hb_face_destroy(face: *mut Face);
    fn hb_font_create(face: *mut Face) -> *mut Font;
    fn hb_font_destroy(font: *mut Font);
    fn hb_font_set_var_coords_normalized(font: *mut Font, coords: *const c_int, length: c_uint);
    fn hb_buffer_create() -> *mut HbBuffer;
    fn hb_buffer_destroy(buffer: *mut HbBuffer);
    fn hb_buffer_clear_contents(buffer: *mut HbBuffer);
    fn hb_buffer_add_utf8(
        buffer: *mut HbBuffer,
        text: *const c_char,
        length: c_int,
        offset: c_uint,
        item_length: c_int,
    );
    fn hb_buffer_set_direction(buffer: *mut HbBuffer, direction: c_int);
    fn hb_buffer_set_script(buffer: *mut HbBuffer, script: u32);
    fn hb_buffer_set_language(buffer: *mut HbBuffer, language: *const HbLanguage);
    fn hb_buffer_set_flags(buffer: *mut HbBuffer, flags: c_uint);
    fn hb_buffer_get_length(buffer: *mut HbBuffer) -> c_uint;
    fn hb_buffer_get_glyph_infos(buffer: *mut HbBuffer, length: *mut c_uint) -> *mut GlyphInfo;
    fn hb_buffer_get_glyph_positions(
        buffer: *mut HbBuffer,
        length: *mut c_uint,
    ) -> *mut GlyphPosition;
    fn hb_glyph_info_get_glyph_flags(info: *const GlyphInfo) -> c_uint;
    fn hb_language_from_string(text: *const c_char, length: c_int) -> *const HbLanguage;
    fn hb_script_from_iso15924_tag(tag: u32) -> u32;
    fn hb_shape(font: *mut Font, buffer: *mut HbBuffer, features: *const Feature, count: c_uint);
}

fn tag(bytes: [u8; 4]) -> u32 {
    u32::from_be_bytes(bytes)
}

pub struct HarfBuzzEngine {
    font: *mut Font,
    buffer: *mut HbBuffer,
    features: Vec<Feature>,
    direction: c_int,
    script: u32,
    language: *const HbLanguage,
    flags: c_uint,
}

impl HarfBuzzEngine {
    pub fn new(prepared: &Prepared) -> Option<Self> {
        let data = prepared.data();
        let length = c_uint::try_from(data.len()).ok()?;
        let font = unsafe {
            let blob = hb_blob_create(
                data.as_ptr().cast(),
                length,
                MEMORY_MODE_READONLY,
                std::ptr::null_mut(),
                None,
            );
            let face = hb_face_create(blob, prepared.face_index());
            hb_blob_destroy(blob);
            let font = hb_font_create(face);
            hb_face_destroy(face);
            font
        };
        let coords: Vec<c_int> = prepared
            .location()
            .coords()
            .iter()
            .map(|coord| c_int::from(coord.0))
            .collect();
        if !coords.is_empty() {
            unsafe {
                hb_font_set_var_coords_normalized(font, coords.as_ptr(), coords.len() as c_uint)
            };
        }
        let properties = prepared.properties;
        let script = <[u8; 4]>::try_from(properties.script.code().as_bytes())
            .map(|code| unsafe { hb_script_from_iso15924_tag(tag(code)) })
            .unwrap_or(0);
        let language = properties.language.map_or(std::ptr::null(), |language| {
            let name = language.as_str();
            unsafe { hb_language_from_string(name.as_ptr().cast(), name.len() as c_int) }
        });
        let mut flags = 0;
        if prepared.flags.contains(BufferFlags::BEGINNING_OF_TEXT) {
            flags |= FLAG_BOT | FLAG_EOT;
        }
        if prepared
            .flags
            .contains(BufferFlags::PRODUCE_UNSAFE_TO_CONCAT)
        {
            flags |= FLAG_PRODUCE_UNSAFE_TO_CONCAT;
        }
        Some(Self {
            font,
            buffer: unsafe { hb_buffer_create() },
            features: prepared
                .features
                .iter()
                .map(|feature| Feature {
                    tag: tag(feature.tag.to_bytes()),
                    value: feature.value,
                    start: feature.start,
                    end: feature.end,
                })
                .collect(),
            direction: match properties.direction {
                Direction::LeftToRight => DIRECTION_LTR,
                Direction::RightToLeft => DIRECTION_RTL,
            },
            script,
            language,
            flags,
        })
    }

    pub fn shape(&mut self, text: &str) -> usize {
        unsafe {
            hb_buffer_clear_contents(self.buffer);
            hb_buffer_add_utf8(
                self.buffer,
                text.as_ptr().cast(),
                text.len() as c_int,
                0,
                text.len() as c_int,
            );
            hb_buffer_set_direction(self.buffer, self.direction);
            if self.script != 0 {
                hb_buffer_set_script(self.buffer, self.script);
            }
            if !self.language.is_null() {
                hb_buffer_set_language(self.buffer, self.language);
            }
            hb_buffer_set_flags(self.buffer, self.flags);
            hb_shape(
                self.font,
                self.buffer,
                self.features.as_ptr(),
                self.features.len() as c_uint,
            );
            hb_buffer_get_length(self.buffer) as usize
        }
    }

    pub fn shape_glyphs(&mut self, text: &str) -> Vec<ShapedGlyph> {
        self.shape(text);
        let mut length = 0;
        let (infos, positions) = unsafe {
            let infos = hb_buffer_get_glyph_infos(self.buffer, &mut length);
            let positions = hb_buffer_get_glyph_positions(self.buffer, &mut length);
            (
                std::slice::from_raw_parts(infos, length as usize),
                std::slice::from_raw_parts(positions, length as usize),
            )
        };
        infos
            .iter()
            .zip(positions)
            .map(|(info, position)| {
                let flags = unsafe { hb_glyph_info_get_glyph_flags(info) };
                ShapedGlyph {
                    glyph: info.codepoint,
                    cluster: info.cluster,
                    x_advance: position.x_advance,
                    y_advance: position.y_advance,
                    x_offset: position.x_offset,
                    y_offset: position.y_offset,
                    unsafe_to_break: flags & GLYPH_FLAG_UNSAFE_TO_BREAK != 0,
                    unsafe_to_concat: flags & GLYPH_FLAG_UNSAFE_TO_CONCAT != 0,
                }
            })
            .collect()
    }
}

impl Drop for HarfBuzzEngine {
    fn drop(&mut self) {
        unsafe {
            hb_buffer_destroy(self.buffer);
            hb_font_destroy(self.font);
        }
    }
}

impl std::fmt::Debug for HarfBuzzEngine {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("HarfBuzzEngine")
            .field("features", &self.features.len())
            .finish_non_exhaustive()
    }
}
