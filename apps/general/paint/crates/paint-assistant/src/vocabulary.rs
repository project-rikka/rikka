use paint_document::{Combine, FileFormat, FloodMode, StackOrder};
use rikka_figure::TextAlign;
use rikka_image_edit::{
    Anchor, Cap, FillRule, Flip, GradientShape, Intent, Join, Profile, QuarterTurn, Resampling,
    Tool as MarkTool,
};
use rikka_psd::BlendMode;
use rikka_types::Color;
use serde_json::Value as Json;

use crate::ops::Parsed;
use crate::order::Sample;

pub(crate) type Fields = &'static [&'static str];

const fn named_all<T: Copy, const N: usize>(
    names: [&'static str; N],
    values: [T; N],
) -> [(&'static str, T); N] {
    let mut named = [(names[0], values[0]); N];
    let mut at = 0;
    while at < N {
        named[at] = (names[at], values[at]);
        at += 1;
    }
    named
}
pub(crate) const MARKS: [(&str, MarkTool); 6] = [
    ("pen", MarkTool::Pen),
    ("highlighter", MarkTool::Highlighter),
    ("line", MarkTool::Line),
    ("arrow", MarkTool::Arrow),
    ("rectangle", MarkTool::Rectangle),
    ("ellipse", MarkTool::Ellipse),
];

pub(crate) const ITEMS: [(&str, Fields); 14] = [
    ("pen", &["points", "color", "width"]),
    ("highlighter", &["points", "color", "width"]),
    ("line", &["points", "color", "width"]),
    ("arrow", &["points", "color", "width"]),
    (
        "rectangle",
        &["rect", "angle", "color", "width", "filling", "fill"],
    ),
    (
        "ellipse",
        &["rect", "angle", "color", "width", "filling", "fill"],
    ),
    (
        "brush",
        &[
            "points",
            "d",
            "color",
            "size",
            "hardness",
            "opacity",
            "flow",
            "antialias",
        ],
    ),
    (
        "eraser",
        &[
            "points",
            "d",
            "color",
            "size",
            "hardness",
            "opacity",
            "flow",
            "antialias",
        ],
    ),
    (
        "path",
        &[
            "d",
            "fill",
            "fill_rule",
            "stroke",
            "width",
            "join",
            "cap",
            "antialias",
        ],
    ),
    ("gradient", &["shape", "from", "to", "colors", "fade"]),
    (
        "text",
        &[
            "text",
            "font",
            "size",
            "weight",
            "italic",
            "align",
            "wrap",
            "antialias",
            "color",
            "at",
            "inside",
            "anchor",
        ],
    ),
    (
        "bucket",
        &["at", "tolerance", "mode", "sample", "antialias", "color"],
    ),
    ("fill", &["color"]),
    ("erase", &["color"]),
];

pub(crate) const SELECTIONS: [(&str, Fields); 15] = [
    ("rectangle", &["rect", "combine", "antialias"]),
    ("ellipse", &["rect", "combine", "antialias"]),
    ("polygon", &["points", "combine", "antialias"]),
    ("path", &["d", "fill_rule", "combine", "antialias"]),
    (
        "wand",
        &["at", "tolerance", "mode", "sample", "antialias", "combine"],
    ),
    ("layer", &["layer", "combine"]),
    ("all", &[]),
    ("none", &[]),
    ("invert", &[]),
    ("expand", &["by"]),
    ("contract", &["by"]),
    ("feather", &["by"]),
    ("border", &["by"]),
    ("smooth", &["by"]),
    ("move", &["dx", "dy"]),
];

pub(crate) const LAYERINGS: [(&str, Fields); 9] = [
    ("new", &["name", "above"]),
    ("duplicate", &["layer", "name"]),
    ("delete", &["layer"]),
    ("move", &["layer", "above", "below", "into", "by"]),
    ("set", &["layer", "name", "visible", "opacity", "blend"]),
    ("merge_down", &["layer"]),
    ("merge_visible", &[]),
    ("flatten", &[]),
    ("from_background", &["name"]),
];

pub(crate) const OBJECTINGS: [(&str, Fields); 7] = [
    ("move", &["objects", "offset"]),
    ("rotate", &["objects", "degrees", "around"]),
    ("set", &["object", "item"]),
    ("delete", &["objects"]),
    ("arrange", &["objects", "stack"]),
    ("duplicate", &["objects", "offset"]),
    ("rasterize", &["layer"]),
];

pub(crate) const STACKINGS: [(&str, StackOrder); 4] = [
    ("front", StackOrder::Front),
    ("forward", StackOrder::Forward),
    ("backward", StackOrder::Backward),
    ("back", StackOrder::Back),
];

pub(crate) const OBJECT_ITEMS: [&str; 6] =
    ["line", "arrow", "rectangle", "ellipse", "path", "text"];

pub(crate) const FILLINGS: [&str; 3] = ["outline", "fill", "both"];

pub(crate) const COMBINES: [(&str, Combine); 5] = [
    ("replace", Combine::Replace),
    ("add", Combine::Union),
    ("subtract", Combine::Exclude),
    ("intersect", Combine::Intersect),
    ("xor", Combine::Xor),
];

pub(crate) const ANCHORS: [(&str, Anchor); 9] = [
    ("top_left", Anchor::TopLeft),
    ("top", Anchor::Top),
    ("top_right", Anchor::TopRight),
    ("left", Anchor::Left),
    ("center", Anchor::Center),
    ("right", Anchor::Right),
    ("bottom_left", Anchor::BottomLeft),
    ("bottom", Anchor::Bottom),
    ("bottom_right", Anchor::BottomRight),
];

pub(crate) const RESAMPLINGS: [(&str, Resampling); 5] = named_all(
    [
        "nearest",
        "bilinear",
        "bicubic",
        "bicubic_smoother",
        "lanczos",
    ],
    Resampling::ALL,
);

pub(crate) const TURNS: [(&str, QuarterTurn); 3] = [
    ("clockwise", QuarterTurn::Clockwise),
    ("counterclockwise", QuarterTurn::CounterClockwise),
    ("half", QuarterTurn::Half),
];

pub(crate) const FLIPS: [(&str, Flip); 2] = [
    ("horizontal", Flip::Horizontal),
    ("vertical", Flip::Vertical),
];

type ProfileFactory = fn() -> Profile;

pub(crate) const PROFILES: [(&str, ProfileFactory); 6] = [
    ("srgb", Profile::srgb),
    ("display_p3", Profile::display_p3),
    ("adobe_rgb", Profile::adobe_rgb),
    ("prophoto_rgb", Profile::pro_photo_rgb),
    ("rec2020", Profile::rec2020),
    ("linear_srgb", Profile::linear_srgb),
];

pub(crate) const INTENTS: [(&str, Intent); 4] = named_all(
    ["perceptual", "relative", "saturation", "absolute"],
    Intent::ALL,
);

pub(crate) const GRADIENTS: [(&str, GradientShape); 5] = [
    ("linear", GradientShape::Linear),
    ("reflected", GradientShape::Reflected),
    ("radial", GradientShape::Radial),
    ("diamond", GradientShape::Diamond),
    ("conical", GradientShape::Conical),
];

pub(crate) const FILL_RULES: [(&str, FillRule); 2] = [
    ("nonzero", FillRule::NonZero),
    ("evenodd", FillRule::EvenOdd),
];

pub(crate) const JOINS: [(&str, Join); 3] = [
    ("miter", Join::Miter),
    ("round", Join::Round),
    ("bevel", Join::Bevel),
];

pub(crate) const CAPS: [(&str, Cap); 3] = [
    ("butt", Cap::Butt),
    ("round", Cap::Round),
    ("square", Cap::Square),
];

pub(crate) const ALIGNS: [(&str, TextAlign); 3] = [
    ("left", TextAlign::Left),
    ("center", TextAlign::Center),
    ("right", TextAlign::Right),
];

pub(crate) const FLOODS: [(&str, FloodMode); 2] = [
    ("contiguous", FloodMode::Contiguous),
    ("global", FloodMode::Global),
];

pub(crate) const SAMPLES: [(&str, Sample); 2] =
    [("image", Sample::Image), ("layer", Sample::Layer)];

pub(crate) const BLENDS: [(&str, BlendMode); 28] = named_all(
    [
        "pass_through",
        "normal",
        "dissolve",
        "darken",
        "multiply",
        "color_burn",
        "linear_burn",
        "darker_color",
        "lighten",
        "screen",
        "color_dodge",
        "linear_dodge",
        "lighter_color",
        "overlay",
        "soft_light",
        "hard_light",
        "vivid_light",
        "linear_light",
        "pin_light",
        "hard_mix",
        "difference",
        "exclusion",
        "subtract",
        "divide",
        "hue",
        "saturation",
        "color",
        "luminosity",
    ],
    BlendMode::ALL,
);

pub(crate) const TRANSFORMS: [&str; 7] = [
    "crop",
    "resize",
    "canvas_size",
    "rotate",
    "turn",
    "flip",
    "convert",
];

pub(crate) const HISTORY: [&str; 4] = ["list", "undo", "redo", "revisit"];

pub fn blend_name(blend: BlendMode) -> &'static str {
    BLENDS
        .iter()
        .find(|(_, known)| *known == blend)
        .map_or("unknown", |(name, _)| name)
}

pub(crate) fn named<T: Copy>(
    table: &[(&'static str, T)],
    requested: &str,
    what: &str,
) -> Parsed<T> {
    table
        .iter()
        .find(|(name, _)| *name == requested)
        .map(|(_, value)| *value)
        .ok_or_else(|| {
            format!(
                "there is no {what} \"{requested}\"; use {}",
                names(table).join(", ")
            )
        })
}

pub(crate) fn names<T>(table: &[(&'static str, T)]) -> Vec<&'static str> {
    table.iter().map(|(name, _)| *name).collect()
}

pub(crate) fn listed<'a>(names: impl IntoIterator<Item = &'a str>) -> String {
    names
        .into_iter()
        .map(|name| format!("`{name}`"))
        .collect::<Vec<String>>()
        .join(", ")
}

pub(crate) fn color(value: &Json) -> Parsed<Color> {
    let text = value.as_str().unwrap_or_default();
    let digits = text.strip_prefix('#').map_or(0, str::len);
    match digits {
        6 | 8 => Color::parse_hex(text),
        _ => None,
    }
    .ok_or_else(|| format!("{value} is no colour; write #rrggbb or #rrggbbaa"))
}

pub(crate) fn formats() -> String {
    let mut extensions: Vec<&str> = FileFormat::writable()
        .into_iter()
        .flat_map(|format| format.extensions().iter().copied())
        .collect();
    extensions.sort_unstable();
    extensions.dedup();
    extensions
        .iter()
        .map(|extension| format!(".{extension}"))
        .collect::<Vec<String>>()
        .join(", ")
}
