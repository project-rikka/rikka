use crate::key::Key;
use crate::schema::Schema;

pub const DOMAIN: &str = "rikka.windows";

pub const LAYOUT: &str = "layout";
pub const GAP_INNER: &str = "gap-inner";
pub const GAP_OUTER: &str = "gap-outer";
pub const RESIZE_ON_BORDER: &str = "resize-on-border";
pub const SNAP: &str = "snap";
pub const SNAP_BAR: &str = "snap-bar";
pub const SNAP_ASSIST: &str = "snap-assist";
pub const SNAP_TOGETHER: &str = "snap-resize-together";
pub const SCROLLING: &str = "scrolling";
pub const FOCUS_OUTLINE: &str = "focus-outline";
pub const ALLOW_TEARING: &str = "allow-tearing";

pub const FLOATING: &str = "floating";
pub const TILING: &str = "tiling";
pub const LAYOUTS: [&str; 2] = [FLOATING, TILING];

pub const LAYOUT_DEFAULT: &str = FLOATING;
pub const GAP_DEFAULT: i64 = 8;
pub const GAP_MAX: i64 = 64;
pub const RESIZE_ON_BORDER_DEFAULT: bool = true;
pub const SNAP_DEFAULT: bool = true;
pub const SCROLLING_DEFAULT: bool = false;
pub const FOCUS_OUTLINE_DEFAULT: bool = true;
pub const ALLOW_TEARING_DEFAULT: bool = true;

#[must_use]
pub fn schema() -> Schema {
    Schema::new(DOMAIN)
        .summary("How the session arranges and presents its windows")
        .key(
            Key::string(LAYOUT, LAYOUT_DEFAULT)
                .summary("Whether windows overlap freely or share the display")
                .choices(LAYOUTS),
        )
        .key(
            Key::int(GAP_INNER, GAP_DEFAULT)
                .summary("Pixels left between two tiled windows")
                .range(0..=GAP_MAX),
        )
        .key(
            Key::int(GAP_OUTER, GAP_DEFAULT)
                .summary("Pixels left between a tiled window and the edge of the display")
                .range(0..=GAP_MAX),
        )
        .key(
            Key::bool(RESIZE_ON_BORDER, RESIZE_ON_BORDER_DEFAULT)
                .summary("Whether dragging the edge of a tiled window resizes it"),
        )
        .key(Key::bool(SCROLLING, SCROLLING_DEFAULT).summary(
            "Whether tiled windows stand in columns on a strip that scrolls sideways \
                 instead of sharing out the display",
        ))
        .key(
            Key::bool(FOCUS_OUTLINE, FOCUS_OUTLINE_DEFAULT)
                .summary("Whether the focused tiled window is framed in the accent colour"),
        )
        .key(Key::bool(SNAP, SNAP_DEFAULT).summary(
            "Whether dragging a floating window to an edge or a snap zone arranges it there",
        ))
        .key(Key::bool(SNAP_BAR, SNAP_DEFAULT).summary(
            "Whether dragging a window to the top of the display reveals the snap layouts",
        ))
        .key(
            Key::bool(SNAP_ASSIST, SNAP_DEFAULT).summary(
                "Whether snapping a window offers the other windows for the zones left over",
            ),
        )
        .key(
            Key::bool(SNAP_TOGETHER, SNAP_DEFAULT)
                .summary("Whether resizing a snapped window resizes the ones beside it"),
        )
        .key(Key::bool(ALLOW_TEARING, ALLOW_TEARING_DEFAULT).summary(
            "Whether a fullscreen window that asks for it shows each frame at once, \
                 tearing, instead of waiting for the display's refresh",
        ))
}
