use rikka_text_font::OutlineScratch;
use rikka_text_unicode::Script;

use crate::buffer::glyph_info::GlyphInfo;
use crate::buffer::glyph_position::GlyphPosition;
use crate::common::buffer_flags::BufferFlags;
use crate::common::direction::Direction;
use crate::common::segment_properties::SegmentProperties;
use crate::face::digest::Digest;

pub(crate) const CONTEXT_LENGTH: usize = 5;
const MAX_LEN_FACTOR: usize = 256;
const MAX_LEN_MIN: usize = 65536;
const MAX_LEN_DEFAULT: usize = 0x3FFF_FFFF;
const MAX_OPS_FACTOR: i32 = 4096;
const MAX_OPS_MIN: i32 = 65536;
const MAX_OPS_DEFAULT: i32 = 0x1FFF_FFFF;

#[derive(Debug)]
pub struct Buffer {
    pub(crate) info: Vec<GlyphInfo>,
    pub(crate) pos: Vec<GlyphPosition>,
    pub(crate) out: Vec<GlyphInfo>,
    pub(crate) len: usize,
    pub(crate) idx: usize,
    pub(crate) out_len: usize,
    pub(crate) have_output: bool,
    pub(crate) have_separate_output: bool,
    pub(crate) have_positions: bool,
    pub(crate) successful: bool,
    pub(crate) shaped: bool,
    pub(crate) direction: Direction,
    pub(crate) script: Script,
    pub(crate) flags: BufferFlags,
    pub(crate) context: [[u32; CONTEXT_LENGTH]; 2],
    pub(crate) context_len: [usize; 2],
    pub(crate) scratch_flags: u32,
    pub(crate) serial: u8,
    pub(crate) digest: Digest,
    pub(crate) max_len: usize,
    pub(crate) max_ops: i32,
    pub(crate) outline_scratch: OutlineScratch,
}

impl Default for Buffer {
    fn default() -> Self {
        Self::new()
    }
}

impl Buffer {
    pub fn new() -> Self {
        Self {
            info: Vec::new(),
            pos: Vec::new(),
            out: Vec::new(),
            len: 0,
            idx: 0,
            out_len: 0,
            have_output: false,
            have_separate_output: false,
            have_positions: false,
            successful: true,
            shaped: false,
            direction: Direction::LeftToRight,
            script: Script::Unknown,
            flags: BufferFlags::empty(),
            context: [[0; CONTEXT_LENGTH]; 2],
            context_len: [0; 2],
            scratch_flags: 0,
            serial: 0,
            digest: Digest::default(),
            max_len: MAX_LEN_DEFAULT,
            max_ops: MAX_OPS_DEFAULT,
            outline_scratch: OutlineScratch::new(),
        }
    }

    pub fn clear(&mut self) {
        self.len = 0;
        self.idx = 0;
        self.out_len = 0;
        self.have_output = false;
        self.have_separate_output = false;
        self.have_positions = false;
        self.successful = true;
        self.shaped = false;
        self.context_len = [0; 2];
        self.scratch_flags = 0;
        self.serial = 0;
    }

    pub fn reserve(&mut self, additional: usize) {
        let _ = self.ensure(self.len + additional);
    }

    pub fn push(&mut self, character: char, cluster: u32) {
        self.start_text();
        if self.ensure(self.len + 1) {
            self.info[self.len] = GlyphInfo::new(character as u32, cluster);
            self.len += 1;
        }
    }

    pub fn push_str(&mut self, text: &str, base_cluster: u32) {
        self.start_text();
        if !self.ensure(self.len + text.len()) {
            return;
        }
        for (offset, character) in text.char_indices() {
            self.info[self.len] =
                GlyphInfo::new(character as u32, base_cluster.wrapping_add(offset as u32));
            self.len += 1;
        }
    }

    pub fn set_pre_context(&mut self, text: &str) {
        self.start_text();
        self.context_len[0] = 0;
        for character in text.chars().rev().take(CONTEXT_LENGTH) {
            self.context[0][self.context_len[0]] = character as u32;
            self.context_len[0] += 1;
        }
    }

    pub fn set_post_context(&mut self, text: &str) {
        self.start_text();
        self.context_len[1] = 0;
        for character in text.chars().take(CONTEXT_LENGTH) {
            self.context[1][self.context_len[1]] = character as u32;
            self.context_len[1] += 1;
        }
    }

    pub fn set_flags(&mut self, flags: BufferFlags) {
        self.flags = flags;
    }

    pub fn flags(&self) -> BufferFlags {
        self.flags
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn guess_properties(&self) -> SegmentProperties {
        let script = self.info[..self.len]
            .iter()
            .map(|info| rikka_text_unicode::script(crate::unicode::to_char(info.codepoint)))
            .find(|script| !matches!(script, Script::Common | Script::Inherited | Script::Unknown))
            .unwrap_or(Script::Unknown);
        SegmentProperties::new(script)
    }

    #[inline]
    pub fn glyph_infos(&self) -> &[GlyphInfo] {
        if self.shaped {
            &self.info[..self.len]
        } else {
            &[]
        }
    }

    #[inline]
    pub fn glyph_positions(&self) -> &[GlyphPosition] {
        if self.shaped {
            &self.pos[..self.len]
        } else {
            &[]
        }
    }

    fn start_text(&mut self) {
        if self.shaped {
            self.clear();
        }
    }

    pub(crate) fn enter(&mut self, direction: Direction, script: Script) {
        self.direction = direction;
        self.script = script;
        self.serial = 0;
        self.scratch_flags = 0;
        self.successful = true;
        self.max_len = self
            .len
            .checked_mul(MAX_LEN_FACTOR)
            .map_or(MAX_LEN_DEFAULT, |len| len.max(MAX_LEN_MIN));
        self.max_ops = i32::try_from(self.len)
            .ok()
            .and_then(|len| len.checked_mul(MAX_OPS_FACTOR))
            .map_or(MAX_OPS_DEFAULT, |ops| ops.max(MAX_OPS_MIN));
    }

    pub(crate) fn leave(&mut self) {
        self.max_len = MAX_LEN_DEFAULT;
        self.max_ops = MAX_OPS_DEFAULT;
        self.serial = 0;
        self.have_output = false;
        self.have_separate_output = false;
        self.shaped = true;
    }

    #[must_use]
    #[inline]
    pub(crate) fn ensure(&mut self, size: usize) -> bool {
        size <= self.info.len() || self.enlarge(size)
    }

    #[cold]
    fn enlarge(&mut self, size: usize) -> bool {
        if size > self.max_len {
            self.successful = false;
            return false;
        }
        let capacity = size.max(32).next_power_of_two().min(self.max_len.max(size));
        self.info.resize(capacity, GlyphInfo::default());
        self.pos.resize(capacity, GlyphPosition::default());
        self.out.resize(capacity, GlyphInfo::default());
        true
    }

    #[inline]
    pub(crate) fn cur(&self, offset: usize) -> &GlyphInfo {
        &self.info[self.idx + offset]
    }

    #[inline]
    pub(crate) fn cur_mut(&mut self, offset: usize) -> &mut GlyphInfo {
        let index = self.idx + offset;
        &mut self.info[index]
    }

    #[inline]
    pub(crate) fn out_info(&self) -> &[GlyphInfo] {
        if self.have_separate_output {
            &self.out
        } else {
            &self.info
        }
    }

    #[inline]
    pub(crate) fn out_info_mut(&mut self) -> &mut [GlyphInfo] {
        if self.have_separate_output {
            &mut self.out
        } else {
            &mut self.info
        }
    }

    #[inline]
    pub(crate) fn prev(&self) -> &GlyphInfo {
        &self.out_info()[self.out_len.saturating_sub(1)]
    }

    #[inline]
    pub(crate) fn prev_mut(&mut self) -> &mut GlyphInfo {
        let index = self.out_len.saturating_sub(1);
        &mut self.out_info_mut()[index]
    }

    #[inline]
    pub(crate) fn backtrack_len(&self) -> usize {
        if self.have_output {
            self.out_len
        } else {
            self.idx
        }
    }

    #[inline]
    pub(crate) fn lookahead_len(&self) -> usize {
        self.len - self.idx
    }

    pub(crate) fn update_digest(&mut self) {
        let mut digest = Digest::default();
        for info in &self.info[..self.len] {
            digest.add(info.codepoint);
        }
        self.digest = digest;
    }

    pub(crate) fn mask_union(&self) -> u32 {
        self.info[..self.len]
            .iter()
            .fold(0, |masks, info| masks | info.mask)
    }

    pub(crate) fn reset_masks(&mut self, mask: u32) {
        for info in &mut self.info[..self.len] {
            info.mask = mask;
        }
    }

    pub(crate) fn set_masks(&mut self, value: u32, mask: u32, start: u32, end: u32) {
        if mask == 0 {
            return;
        }
        let value = value & mask;
        let keep = !mask;
        let global = start == crate::common::feature::GLOBAL_START
            && end == crate::common::feature::GLOBAL_END;
        for info in &mut self.info[..self.len] {
            if global || (start <= info.cluster && info.cluster < end) {
                info.mask = (info.mask & keep) | value;
            }
        }
    }

    pub(crate) fn allocate_lig_id(&mut self) -> u8 {
        loop {
            self.serial = self.serial.wrapping_add(1);
            if self.serial == 0 {
                self.serial = 1;
            }
            let lig_id = self.serial & 0x07;
            if lig_id != 0 {
                return lig_id;
            }
        }
    }
}
