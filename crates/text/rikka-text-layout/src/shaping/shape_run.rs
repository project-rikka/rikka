use std::cell::RefCell;
use std::ops::Range;

use rikka_text_shape::{Buffer, BufferFlags};
use rikka_text_unicode::Script;

use crate::fonts::FaceInstance;
use crate::shaping::classify::{INVISIBLE, classify};
use crate::shaping::columns::{GlyphColumns, flag};
use crate::style::Caps;

thread_local! {
    static BUFFER: RefCell<Buffer> = RefCell::new(Buffer::new());
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Context {
    pub(crate) before: bool,
    pub(crate) after: bool,
}

impl Context {
    pub(crate) const FULL: Self = Self {
        before: true,
        after: true,
    };
}

#[derive(Clone, Debug)]
pub(crate) struct RunRequest<'a> {
    pub(crate) text: &'a str,
    pub(crate) range: Range<usize>,
    pub(crate) face: &'a FaceInstance,
    pub(crate) script: Script,
    pub(crate) rtl: bool,
    pub(crate) context: Context,
    pub(crate) caps: Caps,
}

pub(crate) fn shape_run(request: &RunRequest<'_>, out: &mut GlyphColumns) {
    BUFFER.with(|buffer| shape_with(request, &mut buffer.borrow_mut(), out));
}

fn shape_with(request: &RunRequest<'_>, buffer: &mut Buffer, out: &mut GlyphColumns) {
    let RunRequest {
        text,
        ref range,
        face,
        script,
        rtl,
        context,
        caps,
    } = *request;
    buffer.clear();
    buffer.push_str(&text[range.clone()], range.start as u32);
    if context.before {
        buffer.set_pre_context(&text[..range.start]);
    }
    if context.after {
        buffer.set_post_context(&text[range.end..]);
    }
    let mut flags = BufferFlags::empty();
    if range.start == 0 {
        flags |= BufferFlags::BEGINNING_OF_TEXT;
    }
    if range.end == text.len() {
        flags |= BufferFlags::END_OF_TEXT;
    }
    buffer.set_flags(flags);
    face.plan(script, rtl, caps).shape(buffer);
    let infos = buffer.glyph_infos();
    let positions = buffer.glyph_positions();
    let first = out.len();
    let count = infos.len();
    let order = |step: usize| if rtl { count - 1 - step } else { step };
    let scale = face.units_scale;
    let extra = face.extra_advance;
    out.flags.extend((0..count).map(|step| {
        let info = &infos[order(step)];
        let unsafe_to_break = if info.unsafe_to_break() {
            flag::UNSAFE_TO_BREAK
        } else {
            0
        };
        classify(text, info.cluster() as usize) | unsafe_to_break
    }));
    let flags = &out.flags[first..];
    out.ids.extend((0..count).map(|step| {
        if flags[step] & INVISIBLE != 0 {
            face.space_glyph
        } else {
            infos[order(step)].glyph().0
        }
    }));
    out.advances.extend((0..count).map(|step| {
        let advance = positions[order(step)].x_advance as f32 * scale;
        match (flags[step] & INVISIBLE != 0, advance > 0.0) {
            (true, _) => 0.0,
            (false, true) => advance + extra,
            (false, false) => advance,
        }
    }));
    out.x_offsets
        .extend((0..count).map(|step| positions[order(step)].x_offset as f32 * scale));
    out.y_offsets
        .extend((0..count).map(|step| positions[order(step)].y_offset as f32 * scale));
    out.clusters
        .extend((0..count).map(|step| infos[order(step)].cluster()));
    out.cluster_ends.resize(first + count, 0);
    for flags in &mut out.flags[first..] {
        *flags &= !INVISIBLE;
    }
    fill_cluster_ends(out, first, range.end as u32);
}

fn fill_cluster_ends(out: &mut GlyphColumns, first: usize, end: u32) {
    let mut cluster_end = end;
    let mut current = None;
    for index in (first..out.len()).rev() {
        let cluster = out.clusters[index];
        if current != Some(cluster) {
            if let Some(later) = current {
                cluster_end = later;
            }
            current = Some(cluster);
        }
        out.cluster_ends[index] = cluster_end;
    }
}
