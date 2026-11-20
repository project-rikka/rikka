use std::f32::consts::PI;
use std::ops::Range;

use rikka_text_catalog::catalog;
use rikka_text_layout::{
    Caps, Family, FontStyle, Glyph, LayoutParams, Paragraph, TextStyle, Weight, Width, Wrap,
};
use rikka_types::{Point, Rect};
use rikka_vector::{Antialiasing, Path, Transform, Verb};

use crate::parse::values::{Unit, length};
use crate::render::bounds::placed;
use crate::tree::glyphs::{GlyphInk, Glyphs};
use crate::tree::style::{Axis, Context};
use crate::tree::{Fill, GlyphPicture, Ink, Outline};

const X_HEIGHT_SHARE: f32 = 0.45;
const SUBSCRIPT_SHARE: f32 = 0.2;
const SUPERSCRIPT_SHARE: f32 = 0.4;
const UNDERLINE_SHARE: f32 = 0.1;
const STRIKEOUT_SHARE: f32 = 0.3;
const LINE_THICKNESS_SHARE: f32 = 0.05;
const LEAST_STRETCH: f32 = 0.001;
const CURVE_STEPS: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Anchor {
    Start,
    Middle,
    End,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Baseline {
    Alphabetic,
    Ideographic,
    Hanging,
    Mathematical,
    Central,
    Middle,
    TextBeforeEdge,
    TextAfterEdge,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Shift {
    Subscript,
    Superscript,
    Length(f32),
}

#[derive(Clone, Debug)]
pub(crate) struct Font {
    pub(crate) families: Vec<Family>,
    pub(crate) weight: Weight,
    pub(crate) style: FontStyle,
    pub(crate) width: Width,
    pub(crate) caps: Caps,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DecorationLine {
    Underline,
    Overline,
    LineThrough,
}

#[derive(Clone, Debug)]
pub(crate) struct Decoration {
    pub(crate) line: DecorationLine,
    pub(crate) fill: Option<Fill>,
    pub(crate) stroke: Option<Outline>,
    pub(crate) stroke_first: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct Run {
    pub(crate) font: Font,
    pub(crate) size: f32,
    pub(crate) letter_spacing: f32,
    pub(crate) word_spacing: f32,
    pub(crate) anchor: Anchor,
    pub(crate) baseline: Baseline,
    pub(crate) shifts: Vec<Shift>,
    pub(crate) fill: Option<Fill>,
    pub(crate) stroke: Option<Outline>,
    pub(crate) stroke_first: bool,
    pub(crate) decorations: Vec<Decoration>,
    pub(crate) visible: bool,
    pub(crate) antialiasing: Antialiasing,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Placement {
    pub(crate) x: Option<f32>,
    pub(crate) y: Option<f32>,
    pub(crate) dx: Option<f32>,
    pub(crate) dy: Option<f32>,
    pub(crate) rotate: Option<f32>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Character {
    pub(crate) value: char,
    pub(crate) run: usize,
    pub(crate) flow: Option<usize>,
    pub(crate) shown: bool,
    pub(crate) placement: Placement,
}

#[derive(Clone, Debug)]
pub(crate) struct Flow {
    pub(crate) path: Path,
    pub(crate) start: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Adjust {
    Spacing,
    SpacingAndGlyphs,
}

#[derive(Clone, Debug)]
pub(crate) struct Fit {
    pub(crate) characters: Range<usize>,
    pub(crate) length: f32,
    pub(crate) adjust: Adjust,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Content {
    pub(crate) characters: Vec<Character>,
    pub(crate) runs: Vec<Run>,
    pub(crate) flows: Vec<Flow>,
    pub(crate) fits: Vec<Fit>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Space {
    Collapse,
    Preserve,
}

pub(crate) struct Laid {
    pub(crate) runs: Vec<Typeset>,
    pub(crate) bounds: Option<Rect>,
}

#[derive(Default)]
pub(crate) struct Typeset {
    pub(crate) outlines: Path,
    pub(crate) pictures: Vec<GlyphPicture>,
    pub(crate) lines: Vec<Path>,
}

struct Cluster {
    character: usize,
    run: usize,
    value: char,
    width: f32,
    advance: f32,
    stretch: f32,
    ascent: f32,
    descent: f32,
    glyphs: Vec<(Glyph, Point)>,
}

impl Content {
    pub(crate) fn push(
        &mut self,
        value: char,
        run: usize,
        flow: Option<usize>,
        shown: bool,
        space: Space,
    ) {
        let value = match value {
            '\n' | '\r' | '\t' => ' ',
            other => other,
        };
        if space == Space::Collapse && value == ' ' {
            let after_space = self.characters.last().is_none_or(|last| last.value == ' ');
            if after_space {
                return;
            }
        }
        self.characters.push(Character {
            value,
            run,
            flow,
            shown,
            placement: Placement::default(),
        });
    }

    pub(crate) fn finish(&mut self, space: Space) {
        if space == Space::Collapse && self.characters.last().is_some_and(|last| last.value == ' ')
        {
            self.characters.pop();
        }
    }

    pub(crate) fn place(&mut self, from: usize, lists: &Lists) {
        let rotate_last = lists.rotate.last().copied();
        for (offset, character) in self.characters[from..].iter_mut().enumerate() {
            let placement = &mut character.placement;
            let pick = |list: &[f32]| list.get(offset).copied();
            placement.x = placement.x.or(pick(&lists.x));
            placement.y = placement.y.or(pick(&lists.y));
            placement.dx = placement.dx.or(pick(&lists.dx));
            placement.dy = placement.dy.or(pick(&lists.dy));
            placement.rotate = placement.rotate.or(pick(&lists.rotate).or(rotate_last));
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Lists {
    pub(crate) x: Vec<f32>,
    pub(crate) y: Vec<f32>,
    pub(crate) dx: Vec<f32>,
    pub(crate) dy: Vec<f32>,
    pub(crate) rotate: Vec<f32>,
}

pub(crate) fn lists(context: &Context<'_>) -> Lists {
    let resolved = |name: &str, axis: Axis| -> Vec<f32> {
        let Some(text) = context.attribute(name) else {
            return Vec::new();
        };
        let mut values = Vec::new();
        for piece in
            text.split(|character: char| character == ',' || character.is_ascii_whitespace())
        {
            if piece.is_empty() {
                continue;
            }
            match length(piece) {
                Some(given) => values.push(context.resolve(given, axis)),
                None => return values,
            }
        }
        values
    };
    Lists {
        x: resolved("x", Axis::Horizontal),
        y: resolved("y", Axis::Vertical),
        dx: resolved("dx", Axis::Horizontal),
        dy: resolved("dy", Axis::Vertical),
        rotate: crate::parse::values::numbers(context.attribute("rotate").unwrap_or_default()),
    }
}

pub(crate) fn space(context: &Context<'_>, inherited: Space) -> Space {
    if let Some(value) = context.attribute("xml:space") {
        return match value.trim() {
            "preserve" => Space::Preserve,
            _ => Space::Collapse,
        };
    }
    match context.property("white-space").as_deref() {
        Some("pre" | "pre-wrap" | "break-spaces") => Space::Preserve,
        Some("normal" | "nowrap" | "pre-line") => Space::Collapse,
        _ => inherited,
    }
}

pub(crate) fn font(context: &Context<'_>) -> Font {
    let families = context
        .property("font-family")
        .map(|value| families(&value))
        .filter(|families| !families.is_empty())
        .unwrap_or_else(|| vec![Family::SansSerif]);
    let style = match context.property("font-style").as_deref() {
        Some("italic") => FontStyle::Italic,
        Some("oblique") => FontStyle::OBLIQUE,
        _ => FontStyle::Normal,
    };
    let width = match context.property("font-stretch").as_deref() {
        Some("ultra-condensed") => Width::ULTRA_CONDENSED,
        Some("extra-condensed") => Width::EXTRA_CONDENSED,
        Some("condensed" | "narrower") => Width::CONDENSED,
        Some("semi-condensed") => Width::SEMI_CONDENSED,
        Some("semi-expanded") => Width::SEMI_EXPANDED,
        Some("expanded" | "wider") => Width::EXPANDED,
        Some("extra-expanded") => Width::EXTRA_EXPANDED,
        Some("ultra-expanded") => Width::ULTRA_EXPANDED,
        Some(other) => other
            .strip_suffix('%')
            .and_then(|percent| percent.trim().parse::<f32>().ok())
            .filter(|percent| *percent > 0.0)
            .map_or(Width::NORMAL, Width),
        None => Width::NORMAL,
    };
    Font {
        families,
        weight: weight(context),
        style,
        width,
        caps: caps(context),
    }
}

fn caps(context: &Context<'_>) -> Caps {
    let named = |value: &str| {
        value.split_ascii_whitespace().find_map(|word| match word {
            "small-caps" => Some(Caps::Small),
            "all-small-caps" => Some(Caps::AllSmall),
            "petite-caps" => Some(Caps::Petite),
            "all-petite-caps" => Some(Caps::AllPetite),
            "unicase" => Some(Caps::Unicase),
            "titling-caps" => Some(Caps::Titling),
            _ => None,
        })
    };
    context
        .property("font-variant-caps")
        .as_deref()
        .and_then(named)
        .or_else(|| context.property("font-variant").as_deref().and_then(named))
        .unwrap_or(Caps::Normal)
}

pub(crate) fn lines(context: &Context<'_>) -> Vec<DecorationLine> {
    let mut found = Vec::new();
    for name in ["text-decoration-line", "text-decoration"] {
        let Some(value) = context.attribute(name) else {
            continue;
        };
        for word in value.split_ascii_whitespace() {
            let line = match word {
                "underline" => DecorationLine::Underline,
                "overline" => DecorationLine::Overline,
                "line-through" => DecorationLine::LineThrough,
                _ => continue,
            };
            if !found.contains(&line) {
                found.push(line);
            }
        }
    }
    found
}

pub(crate) fn text_length(context: &Context<'_>, characters: Range<usize>) -> Option<Fit> {
    let length = context.coordinate("textLength", Axis::Horizontal)?;
    if !(length.is_finite() && length >= 0.0) || characters.is_empty() {
        return None;
    }
    let adjust = match context.attribute("lengthAdjust").map(str::trim) {
        Some("spacingAndGlyphs") => Adjust::SpacingAndGlyphs,
        _ => Adjust::Spacing,
    };
    Some(Fit {
        characters,
        length,
        adjust,
    })
}

fn families(value: &str) -> Vec<Family> {
    value
        .split(',')
        .map(|name| name.trim().trim_matches(['"', '\'']).trim())
        .filter(|name| !name.is_empty())
        .map(Family::parse)
        .collect()
}

fn weight(context: &Context<'_>) -> Weight {
    let mut weight = 400.0f32;
    for value in context.ancestry("font-weight") {
        weight = match value.trim() {
            "normal" => 400.0,
            "bold" => 700.0,
            "bolder" => match weight {
                w if w < 350.0 => 400.0,
                w if w < 550.0 => 700.0,
                w if w < 900.0 => 900.0,
                w => w,
            },
            "lighter" => match weight {
                w if w < 100.0 => w,
                w if w < 550.0 => 100.0,
                w if w < 750.0 => 400.0,
                _ => 700.0,
            },
            number => number
                .parse::<f32>()
                .ok()
                .filter(|number| (1.0..=1000.0).contains(number))
                .unwrap_or(weight),
        };
    }
    Weight(weight)
}

pub(crate) fn anchor(context: &Context<'_>) -> Anchor {
    match context.property("text-anchor").as_deref() {
        Some("middle") => Anchor::Middle,
        Some("end") => Anchor::End,
        _ => Anchor::Start,
    }
}

pub(crate) fn baseline(context: &Context<'_>) -> Baseline {
    let named = |value: &str| match value {
        "ideographic" => Some(Baseline::Ideographic),
        "hanging" => Some(Baseline::Hanging),
        "mathematical" => Some(Baseline::Mathematical),
        "central" => Some(Baseline::Central),
        "middle" => Some(Baseline::Middle),
        "text-before-edge" | "before-edge" | "text-top" => Some(Baseline::TextBeforeEdge),
        "text-after-edge" | "after-edge" | "text-bottom" => Some(Baseline::TextAfterEdge),
        "alphabetic" => Some(Baseline::Alphabetic),
        _ => None,
    };
    context
        .attribute("alignment-baseline")
        .and_then(named)
        .or_else(|| {
            context
                .property("dominant-baseline")
                .as_deref()
                .and_then(named)
        })
        .unwrap_or(Baseline::Alphabetic)
}

pub(crate) fn shift(context: &Context<'_>) -> Option<Shift> {
    let value = context.attribute("baseline-shift")?;
    match value.trim() {
        "sub" => Some(Shift::Subscript),
        "super" => Some(Shift::Superscript),
        "baseline" => None,
        other => {
            let given = length(other)?;
            let size = context.font_size();
            Some(Shift::Length(match given.unit {
                Unit::Percent => size * given.value / 100.0,
                _ => context.resolve(given, Axis::Vertical),
            }))
        }
    }
}

struct Metrics {
    ascent: f32,
    descent: f32,
    x_height: f32,
    subscript: f32,
    superscript: f32,
    underline: f32,
    underline_thickness: f32,
    strikeout: f32,
    strikeout_thickness: f32,
}

impl Metrics {
    fn line(&self, line: DecorationLine) -> (f32, f32) {
        match line {
            DecorationLine::Underline => (self.underline, self.underline_thickness),
            DecorationLine::Overline => (-self.ascent, self.underline_thickness),
            DecorationLine::LineThrough => (self.strikeout, self.strikeout_thickness),
        }
    }
}

fn metrics(glyph: Option<&Glyph>, size: f32) -> Metrics {
    let fallback = Metrics {
        ascent: size * 0.8,
        descent: -size * 0.2,
        x_height: size * X_HEIGHT_SHARE,
        subscript: size * SUBSCRIPT_SHARE,
        superscript: size * SUPERSCRIPT_SHARE,
        underline: size * UNDERLINE_SHARE,
        underline_thickness: size * LINE_THICKNESS_SHARE,
        strikeout: -size * STRIKEOUT_SHARE,
        strikeout_thickness: size * LINE_THICKNESS_SHARE,
    };
    let Some(glyph) = glyph else {
        return fallback;
    };
    let Some(font) = catalog().font(glyph.face) else {
        return fallback;
    };
    let metrics = font.metrics(glyph.location.location());
    let scale = size / f32::from(metrics.units_per_em.max(1));
    let ascent = metrics.ascender * scale;
    let descent = metrics.descender * scale;
    let placements = font
        .os2()
        .map(|os2| (os2.subscript().y_offset, os2.superscript().y_offset));
    Metrics {
        ascent,
        descent,
        x_height: metrics
            .x_height
            .filter(|height| *height > 0.0)
            .map_or((ascent - descent) * X_HEIGHT_SHARE, |height| height * scale),
        subscript: placements.map_or(fallback.subscript, |(subscript, _)| {
            f32::from(subscript) * scale
        }),
        superscript: placements.map_or(fallback.superscript, |(_, superscript)| {
            f32::from(superscript) * scale
        }),
        underline: -metrics.underline_position * scale,
        underline_thickness: metrics.underline_thickness * scale,
        strikeout: -metrics.strikeout_position * scale,
        strikeout_thickness: metrics.strikeout_thickness * scale,
    }
}

fn baseline_offset(run: &Run, metrics: &Metrics) -> f32 {
    let shifted: f32 = run
        .shifts
        .iter()
        .map(|shift| match shift {
            Shift::Subscript => -metrics.subscript,
            Shift::Superscript => metrics.superscript,
            Shift::Length(length) => *length,
        })
        .sum();
    let aligned = match run.baseline {
        Baseline::Alphabetic => 0.0,
        Baseline::TextBeforeEdge => metrics.ascent,
        Baseline::Middle => metrics.x_height * 0.5,
        Baseline::Central => metrics.ascent - (metrics.ascent - metrics.descent) * 0.5,
        Baseline::TextAfterEdge | Baseline::Ideographic => metrics.descent,
        Baseline::Hanging => metrics.ascent * 0.8,
        Baseline::Mathematical => metrics.ascent * 0.5,
    };
    aligned - shifted
}

fn spaces_letters(value: char) -> bool {
    !matches!(
        u32::from(value),
        0x0600..=0x074F | 0x0750..=0x077F | 0x07C0..=0x07FF | 0x0800..=0x08FF | 0x1800..=0x18AF
            | 0xFB50..=0xFDFF | 0xFE70..=0xFEFF
    )
}

fn separates_words(value: char) -> bool {
    matches!(
        u32::from(value),
        0x0020 | 0x00A0 | 0x1361 | 0x10100 | 0x10101 | 0x1039F | 0x1091F
    )
}

fn shaped(content: &Content, characters: &[usize]) -> Vec<Cluster> {
    let mut clusters = Vec::new();
    let mut start = 0;
    while start < characters.len() {
        let run = content.characters[characters[start]].run;
        let end = characters[start..]
            .iter()
            .position(|index| content.characters[*index].run != run)
            .map_or(characters.len(), |length| start + length);
        let segment = &characters[start..end];
        let mut text = String::new();
        let mut owners = Vec::new();
        for index in segment {
            owners.push((text.len(), *index));
            text.push(content.characters[*index].value);
        }
        let style = &content.runs[run];
        let text_style = TextStyle::new(style.font.families.iter().cloned())
            .weight(style.font.weight)
            .style(style.font.style)
            .width(style.font.width)
            .caps(style.font.caps);
        let shaped = Paragraph::new(&text, &text_style).shape();
        let layout = shaped.layout(&LayoutParams {
            wrap: Wrap::None,
            ..LayoutParams::new(style.size, style.size)
        });
        let glyphs = layout.all_glyphs();
        let metrics = metrics(glyphs.first(), style.size);
        let owner = |byte: u32| {
            let at = owners.partition_point(|(offset, _)| *offset <= byte as usize);
            owners[at.saturating_sub(1)].1
        };
        let mut pen = 0.0;
        let mut at = 0;
        while at < glyphs.len() {
            let cluster_start = glyphs[at].start;
            let origin = pen;
            let mut cluster = Cluster {
                character: owner(cluster_start),
                run,
                value: content.characters[owner(cluster_start)].value,
                width: 0.0,
                advance: 0.0,
                stretch: 1.0,
                ascent: metrics.ascent,
                descent: metrics.descent,
                glyphs: Vec::new(),
            };
            while at < glyphs.len() && glyphs[at].start == cluster_start {
                let glyph = glyphs[at];
                cluster
                    .glyphs
                    .push((glyph, Point::new(glyph.x - origin, glyph.y)));
                pen += glyph.advance;
                cluster.width += glyph.advance;
                at += 1;
            }
            cluster.advance = cluster.width;
            clusters.push(cluster);
        }
        start = end;
    }
    let count = clusters.len();
    for (index, cluster) in clusters.iter_mut().enumerate() {
        let run = &content.runs[cluster.run];
        if index + 1 != count && spaces_letters(cluster.value) {
            cluster.advance += run.letter_spacing;
        }
        if separates_words(cluster.value) {
            cluster.advance += run.word_spacing;
        }
        if cluster.advance < 0.0 && run.letter_spacing < 0.0 {
            cluster.advance = 0.0;
            cluster.glyphs.clear();
        }
    }
    clusters
}

fn fit_lengths(content: &Content, clusters: &mut [Cluster]) {
    for fit in &content.fits {
        let members: Vec<usize> = (0..clusters.len())
            .filter(|index| fit.characters.contains(&clusters[*index].character))
            .collect();
        let Some((&last, rest)) = members.split_last() else {
            continue;
        };
        let measured = rest
            .iter()
            .map(|index| clusters[*index].advance)
            .sum::<f32>()
            + clusters[last].width;
        match fit.adjust {
            Adjust::Spacing => {
                if rest.is_empty() {
                    continue;
                }
                let gap = (fit.length - measured) / rest.len() as f32;
                for index in rest {
                    clusters[*index].advance += gap;
                }
            }
            Adjust::SpacingAndGlyphs => {
                let factor = fit.length / measured;
                if !(factor.is_finite() && factor >= LEAST_STRETCH) {
                    continue;
                }
                for index in &members {
                    let cluster = &mut clusters[*index];
                    cluster.stretch *= factor;
                    cluster.width *= factor;
                    cluster.advance *= factor;
                }
            }
        }
    }
}

struct Span {
    run: usize,
    origin: Transform,
    width: f32,
    metrics: Metrics,
}

pub(crate) fn lay_out(content: &Content, glyphs: &mut Glyphs) -> Laid {
    let mut laid = Laid {
        runs: content
            .runs
            .iter()
            .map(|run| Typeset {
                lines: vec![Path::new(); run.decorations.len()],
                ..Typeset::default()
            })
            .collect(),
        bounds: None,
    };
    let inks: Vec<Option<GlyphInk>> = content.runs.iter().map(ink).collect();
    let shown: Vec<usize> = (0..content.characters.len())
        .filter(|index| content.characters[*index].shown)
        .collect();
    let mut last = Point::default();
    let mut start = 0;
    while start < shown.len() {
        let first = &content.characters[shown[start]];
        let end = shown[start + 1..]
            .iter()
            .position(|index| {
                let character = &content.characters[*index];
                character.placement.x.is_some()
                    || character.placement.y.is_some()
                    || character.flow != first.flow
            })
            .map_or(shown.len(), |length| start + 1 + length);
        let chunk = &shown[start..end];
        start = end;
        let mut clusters = shaped(content, chunk);
        if clusters.is_empty() {
            continue;
        }
        fit_lengths(content, &mut clusters);
        let total: f32 = clusters.iter().map(|cluster| cluster.advance).sum();
        let anchored = match content.runs[first.run].anchor {
            Anchor::Start => 0.0,
            Anchor::Middle => -total / 2.0,
            Anchor::End => -total,
        };
        let placed_clusters = match first.flow {
            None => {
                let origin = Point::new(
                    first.placement.x.unwrap_or(last.x),
                    first.placement.y.unwrap_or(last.y),
                );
                let (transforms, end) = linear(content, &clusters, origin, anchored);
                last = end;
                transforms
            }
            Some(flow) => {
                let offset =
                    first.placement.x.unwrap_or(0.0) + content.flows[flow].start + anchored;
                let (transforms, end) =
                    along(content, &clusters, &content.flows[flow].path, offset);
                last = end;
                transforms
            }
        };
        let mut open: Option<Span> = None;
        for (cluster, transform) in clusters.iter().zip(placed_clusters) {
            let run = &content.runs[cluster.run];
            let Some(transform) = transform.filter(|_| run.visible) else {
                if let Some(transform) = transform {
                    extend_bounds(&mut laid.bounds, cluster, &transform);
                }
                decorate(content, open.take(), &mut laid);
                continue;
            };
            extend_bounds(&mut laid.bounds, cluster, &transform);
            let typeset = &mut laid.runs[cluster.run];
            for (glyph, offset) in &cluster.glyphs {
                let placement = Transform::translate(offset.x, offset.y)
                    .then(Transform::scale(cluster.stretch, 1.0))
                    .then(transform);
                glyphs.place(
                    glyph,
                    &placement,
                    inks[cluster.run].as_ref(),
                    &mut typeset.outlines,
                    &mut typeset.pictures,
                );
            }
            if run.decorations.is_empty() {
                decorate(content, open.take(), &mut laid);
                continue;
            }
            let placement = &content.characters[cluster.character].placement;
            let moved = |value: Option<f32>| value.is_some_and(|value| value != 0.0);
            let shifted = moved(placement.dx) || moved(placement.dy) || moved(placement.rotate);
            match open.as_mut() {
                Some(span) if span.run == cluster.run && first.flow.is_none() && !shifted => {
                    span.width += cluster.advance;
                }
                _ => {
                    decorate(content, open.take(), &mut laid);
                    open = Some(Span {
                        run: cluster.run,
                        origin: transform,
                        width: cluster.advance,
                        metrics: metrics_of(cluster, run.size),
                    });
                }
            }
        }
        decorate(content, open.take(), &mut laid);
    }
    laid
}

fn extend_bounds(bounds: &mut Option<Rect>, cluster: &Cluster, transform: &Transform) {
    let cell = Rect::new(
        0.0,
        -cluster.ascent,
        cluster.advance,
        cluster.ascent - cluster.descent,
    );
    let cell = placed(cell, transform);
    *bounds = Some(bounds.map_or(cell, |known| known.union(&cell)));
}

fn decorate(content: &Content, span: Option<Span>, laid: &mut Laid) {
    let Some(span) = span else {
        return;
    };
    let run = &content.runs[span.run];
    for (decoration, path) in run.decorations.iter().zip(&mut laid.runs[span.run].lines) {
        let (offset, thickness) = span.metrics.line(decoration.line);
        let rect = Rect::new(0.0, offset - thickness / 2.0, span.width, thickness);
        let finite = [offset, thickness, span.width]
            .iter()
            .all(|value| value.is_finite());
        if !finite || !(span.width > 0.0 && thickness > 0.0) {
            continue;
        }
        let [first, rest @ ..] = rect.corners().map(|corner| span.origin.apply(corner));
        path.move_to(first);
        for corner in rest {
            path.line_to(corner);
        }
        path.close();
    }
}

fn ink(run: &Run) -> Option<GlyphInk> {
    let fill = run.fill.as_ref()?;
    let [r, g, b, a] = match &fill.ink {
        Ink::Color(color) => color.to_rgba8(),
        Ink::Gradient(_) | Ink::Pattern(_) => [0, 0, 0, 255],
    };
    Some(GlyphInk {
        foreground: rikka_text_font::Color { r, g, b, a },
        opacity: fill.opacity,
    })
}

fn linear(
    content: &Content,
    clusters: &[Cluster],
    origin: Point,
    anchored: f32,
) -> (Vec<Option<Transform>>, Point) {
    let (mut x, mut y) = (anchored, 0.0);
    let mut transforms = Vec::with_capacity(clusters.len());
    for cluster in clusters {
        let character = &content.characters[cluster.character];
        x += character.placement.dx.unwrap_or(0.0);
        y += character.placement.dy.unwrap_or(0.0);
        let run = &content.runs[cluster.run];
        let shift = baseline_offset(run, &metrics_of(cluster, run.size));
        let transform = Transform::rotate(character.placement.rotate.unwrap_or(0.0))
            .then(Transform::translate(x, y))
            .then(Transform::translate(0.0, shift))
            .then(Transform::translate(origin.x, origin.y));
        transforms.push(Some(transform));
        x += cluster.advance;
    }
    (transforms, Point::new(origin.x + x, origin.y + y))
}

fn metrics_of(cluster: &Cluster, size: f32) -> Metrics {
    let mut found = metrics(cluster.glyphs.first().map(|(glyph, _)| glyph), size);
    found.ascent = cluster.ascent;
    found.descent = cluster.descent;
    found
}

pub(crate) fn path_length(path: &Path) -> f32 {
    Polyline::of(path).lengths.last().copied().unwrap_or(0.0)
}

struct Polyline {
    points: Vec<Point>,
    lengths: Vec<f32>,
}

impl Polyline {
    fn of(path: &Path) -> Polyline {
        let mut points: Vec<Point> = Vec::new();
        let mut breaks = Vec::new();
        let source = path.points();
        let (mut at, mut start) = (0, Point::default());
        for verb in path.verbs() {
            match verb {
                Verb::Move => {
                    start = source[at];
                    breaks.push(points.len());
                    points.push(start);
                    at += 1;
                }
                Verb::Line => {
                    points.push(source[at]);
                    at += 1;
                }
                Verb::Quad => {
                    let from = points.last().copied().unwrap_or(start);
                    let (control, to) = (source[at], source[at + 1]);
                    for step in 1..=CURVE_STEPS {
                        let t = step as f32 / CURVE_STEPS as f32;
                        let u = 1.0 - t;
                        points.push(from * (u * u) + control * (2.0 * u * t) + to * (t * t));
                    }
                    at += 2;
                }
                Verb::Cubic => {
                    let from = points.last().copied().unwrap_or(start);
                    let (first, second, to) = (source[at], source[at + 1], source[at + 2]);
                    for step in 1..=CURVE_STEPS {
                        let t = step as f32 / CURVE_STEPS as f32;
                        let u = 1.0 - t;
                        points.push(
                            from * (u * u * u)
                                + first * (3.0 * u * u * t)
                                + second * (3.0 * u * t * t)
                                + to * (t * t * t),
                        );
                    }
                    at += 3;
                }
                Verb::Close => points.push(start),
            }
        }
        let mut lengths = Vec::with_capacity(points.len());
        let mut total = 0.0;
        for index in 0..points.len() {
            if index > 0 && !breaks.contains(&index) {
                total += points[index].distance(points[index - 1]);
            }
            lengths.push(total);
        }
        Polyline { points, lengths }
    }

    fn at(&self, distance: f32) -> Option<(Point, f32)> {
        let total = *self.lengths.last()?;
        if !(0.0..=total).contains(&distance) {
            return None;
        }
        let index = self
            .lengths
            .partition_point(|length| *length < distance)
            .clamp(1, self.points.len() - 1);
        let (from, to) = (self.points[index - 1], self.points[index]);
        let span = self.lengths[index] - self.lengths[index - 1];
        let share = match span > 0.0 {
            true => (distance - self.lengths[index - 1]) / span,
            false => 0.0,
        };
        let delta = to - from;
        Some((from + delta * share, delta.y.atan2(delta.x) * 180.0 / PI))
    }
}

fn along(
    content: &Content,
    clusters: &[Cluster],
    path: &Path,
    offset: f32,
) -> (Vec<Option<Transform>>, Point) {
    let line = Polyline::of(path);
    let mut transforms = Vec::with_capacity(clusters.len());
    let (mut advance, mut dy) = (offset, 0.0);
    let mut last = Point::default();
    for cluster in clusters {
        let character = &content.characters[cluster.character];
        advance += character.placement.dx.unwrap_or(0.0);
        dy += character.placement.dy.unwrap_or(0.0);
        let half = cluster.width / 2.0;
        let middle = advance + half;
        advance += cluster.advance;
        let Some((point, angle)) = line.at(middle) else {
            transforms.push(None);
            continue;
        };
        let run = &content.runs[cluster.run];
        let shift = baseline_offset(run, &metrics_of(cluster, run.size));
        let transform = Transform::rotate(character.placement.rotate.unwrap_or(0.0))
            .then(Transform::translate(0.0, dy + shift))
            .then(Transform::translate(-half, 0.0))
            .then(Transform::rotate(angle))
            .then(Transform::translate(point.x, point.y));
        transforms.push(Some(transform));
        last = Point::new(point.x + cluster.advance, point.y);
    }
    (transforms, last)
}
