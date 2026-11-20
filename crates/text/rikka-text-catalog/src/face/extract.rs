use rikka_text_font::{Font, Name, NameId, Selection, Tag};

use crate::coverage::{CoverageBuilder, Page};
use crate::face::attributes::{Axis, AxisRange, ColorFormats, Slope};
use crate::face::record::FaceRecord;
use crate::family::fold;

const WEIGHT_AXIS: Tag = Tag::new(b"wght");
const WIDTH_AXIS: Tag = Tag::new(b"wdth");
const ITALIC_AXIS: Tag = Tag::new(b"ital");
const SLANT_AXIS: Tag = Tag::new(b"slnt");
const DEFAULT_SCRIPT: Tag = Tag::new(b"DFLT");
const FAMILY_NAMES: [NameId; 3] = [
    NameId::TYPOGRAPHIC_FAMILY,
    NameId::FAMILY,
    NameId::WWS_FAMILY,
];
const WIDTH_CLASSES: [f32; 9] = [50.0, 62.5, 75.0, 87.5, 100.0, 112.5, 125.0, 150.0, 200.0];
const DEFAULT_OBLIQUE_ANGLE: f32 = 14.0;
const PANOSE_LATIN_TEXT: u8 = 2;
const PANOSE_MONOSPACED: u8 = 9;

pub(crate) fn extract(
    font: &Font<'_>,
    file_stem: &str,
    coverage: &mut CoverageBuilder,
) -> (FaceRecord, Vec<(u16, Page)>) {
    let name = font.name();
    let get = |id: NameId| name.as_ref().and_then(|name| name.get(id));
    let families = families(name.as_ref(), file_stem);
    let style_name = get(NameId::TYPOGRAPHIC_SUBFAMILY)
        .or_else(|| get(NameId::SUBFAMILY))
        .unwrap_or_else(|| "Regular".to_owned());
    let full_name =
        get(NameId::FULL_NAME).unwrap_or_else(|| format!("{} {style_name}", families[0]));
    let axes: Vec<Axis> = font
        .fvar()
        .map(|fvar| {
            fvar.axes()
                .map(|axis| Axis {
                    tag: axis.tag,
                    min: axis.min,
                    default: axis.default,
                    max: axis.max,
                    hidden: axis.hidden,
                })
                .collect()
        })
        .unwrap_or_default();
    if let Some(charmap) = font.charmap() {
        charmap.for_each_mapping(|codepoint, _| coverage.add(codepoint));
    }
    let record = FaceRecord {
        index: font.face_index(),
        style_name: style_name.as_str().into(),
        full_name: full_name.into(),
        weight: weight(font, &axes),
        width: width(font, &axes),
        slope: slope(font, &style_name, &axes),
        monospace: is_monospace(font),
        color: color_formats(font),
        scripts: scripts(font),
        families,
        axes,
        coverage: Vec::new(),
    };
    (record, coverage.finish())
}

fn families(name: Option<&Name<'_>>, file_stem: &str) -> Vec<Box<str>> {
    let mut families = FamilyNames::default();
    if let Some(name) = name {
        if let Some(primary) = name
            .get(NameId::TYPOGRAPHIC_FAMILY)
            .or_else(|| name.get(NameId::FAMILY))
        {
            families.push(&primary);
        }
        for id in FAMILY_NAMES {
            for record in name.records() {
                if record.name_id == id && record.is_decodable() {
                    families.push(&record.to_string());
                }
            }
        }
    }
    if families.names.is_empty() {
        families.push(file_stem);
    }
    if families.names.is_empty() {
        families.push("Unnamed");
    }
    families.names
}

#[derive(Default)]
struct FamilyNames {
    names: Vec<Box<str>>,
    folded: Vec<String>,
}

impl FamilyNames {
    fn push(&mut self, family: &str) {
        let key = fold(family);
        if !key.is_empty() && !self.folded.contains(&key) {
            self.folded.push(key);
            self.names.push(family.trim().into());
        }
    }
}

fn axis(axes: &[Axis], tag: Tag) -> Option<&Axis> {
    axes.iter().find(|axis| axis.tag == tag)
}

fn weight(font: &Font<'_>, axes: &[Axis]) -> AxisRange {
    if let Some(axis) = axis(axes, WEIGHT_AXIS) {
        let mut range = AxisRange::from(axis);
        range.min = range.min.clamp(1.0, 1000.0);
        range.max = range.max.clamp(range.min, 1000.0);
        range.default = range.default.clamp(range.min, range.max);
        return range;
    }
    let class = font.os2().map_or(400, |os2| os2.weight_class());
    let class = match class {
        0 => 400,
        1..=9 => class * 100,
        _ => class.min(1000),
    };
    AxisRange::fixed(f32::from(class))
}

fn width(font: &Font<'_>, axes: &[Axis]) -> AxisRange {
    if let Some(axis) = axis(axes, WIDTH_AXIS) {
        return AxisRange::from(axis);
    }
    let class = font.os2().map_or(5, |os2| os2.width_class());
    let percent = WIDTH_CLASSES
        .get(usize::from(class).wrapping_sub(1))
        .copied()
        .unwrap_or(100.0);
    AxisRange::fixed(percent)
}

fn slope(font: &Font<'_>, style_name: &str, axes: &[Axis]) -> Slope {
    let style = style_name.to_ascii_lowercase();
    let selection = font.os2().map(|os2| os2.selection()).unwrap_or_default();
    let italic_angle = font.post().map_or(0.0, |post| post.italic_angle());
    let oblique = selection.contains(Selection::OBLIQUE) || style.contains("oblique");
    let italic = selection.contains(Selection::ITALIC)
        || font.head().is_some_and(|head| head.is_italic())
        || style.contains("italic");
    let mut slope = if oblique {
        let angle = if -italic_angle > 0.0 {
            -italic_angle
        } else {
            DEFAULT_OBLIQUE_ANGLE
        };
        Slope {
            upright: false,
            italic: false,
            oblique: Some((angle, angle)),
        }
    } else if italic {
        Slope {
            upright: false,
            italic: true,
            oblique: None,
        }
    } else {
        Slope::UPRIGHT
    };
    if let Some(ital) = axis(axes, ITALIC_AXIS) {
        slope.italic |= ital.max >= 1.0;
        slope.upright = ital.min <= 0.0;
    }
    if let Some(slnt) = axis(axes, SLANT_AXIS) {
        let steepest = -slnt.min;
        if steepest > 0.0 {
            slope.oblique = Some(((-slnt.max).max(0.0), steepest));
        }
        slope.upright |= slnt.min <= 0.0 && 0.0 <= slnt.max;
    }
    slope
}

fn is_monospace(font: &Font<'_>) -> bool {
    font.post().is_some_and(|post| post.is_fixed_pitch())
        || font.os2().is_some_and(|os2| {
            let panose = os2.panose();
            panose[0] == PANOSE_LATIN_TEXT && panose[3] == PANOSE_MONOSPACED
        })
}

fn color_formats(font: &Font<'_>) -> ColorFormats {
    let has = |tag: &[u8; 4]| font.has_table(Tag::new(tag));
    ColorFormats {
        colr: has(b"COLR") && has(b"CPAL"),
        cbdt: has(b"CBDT") && has(b"CBLC"),
        sbix: has(b"sbix"),
        svg: has(b"SVG "),
    }
}

fn scripts(font: &Font<'_>) -> Vec<Tag> {
    let mut tags: Vec<Tag> = font
        .gsub()
        .into_iter()
        .flat_map(|gsub| gsub.scripts().tags())
        .chain(
            font.gpos()
                .into_iter()
                .flat_map(|gpos| gpos.scripts().tags()),
        )
        .filter(|&tag| tag != DEFAULT_SCRIPT)
        .collect();
    tags.sort_unstable();
    tags.dedup();
    tags
}
