use crate::resize_edge::ResizeEdge;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum CursorShape {
    #[default]
    Default,
    Hidden,
    ContextMenu,
    Help,
    Pointer,
    Progress,
    Wait,
    Cell,
    Crosshair,
    Text,
    VerticalText,
    Alias,
    Copy,
    Move,
    NoDrop,
    NotAllowed,
    Grab,
    Grabbing,
    AllScroll,
    Resize(ResizeEdge),
    ResizeColumn,
    ResizeRow,
    ResizeEastWest,
    ResizeNorthSouth,
    ResizeNorthEastSouthWest,
    ResizeNorthWestSouthEast,
    ZoomIn,
    ZoomOut,
    Image(CursorImage),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CursorImage(u64);

impl CursorImage {
    #[must_use]
    pub const fn new(id: u64) -> CursorImage {
        CursorImage(id)
    }

    #[must_use]
    pub const fn id(self) -> u64 {
        self.0
    }
}

impl CursorShape {
    #[must_use]
    pub fn from_css(name: &str) -> Option<CursorShape> {
        Some(match name {
            "default" | "auto" => CursorShape::Default,
            "none" => CursorShape::Hidden,
            "context-menu" => CursorShape::ContextMenu,
            "help" => CursorShape::Help,
            "pointer" => CursorShape::Pointer,
            "progress" => CursorShape::Progress,
            "wait" => CursorShape::Wait,
            "cell" => CursorShape::Cell,
            "crosshair" => CursorShape::Crosshair,
            "text" => CursorShape::Text,
            "vertical-text" => CursorShape::VerticalText,
            "alias" => CursorShape::Alias,
            "copy" => CursorShape::Copy,
            "move" => CursorShape::Move,
            "no-drop" => CursorShape::NoDrop,
            "not-allowed" => CursorShape::NotAllowed,
            "grab" => CursorShape::Grab,
            "grabbing" => CursorShape::Grabbing,
            "all-scroll" => CursorShape::AllScroll,
            "n-resize" => CursorShape::Resize(ResizeEdge::Top),
            "s-resize" => CursorShape::Resize(ResizeEdge::Bottom),
            "w-resize" => CursorShape::Resize(ResizeEdge::Left),
            "e-resize" => CursorShape::Resize(ResizeEdge::Right),
            "nw-resize" => CursorShape::Resize(ResizeEdge::TopLeft),
            "ne-resize" => CursorShape::Resize(ResizeEdge::TopRight),
            "sw-resize" => CursorShape::Resize(ResizeEdge::BottomLeft),
            "se-resize" => CursorShape::Resize(ResizeEdge::BottomRight),
            "col-resize" => CursorShape::ResizeColumn,
            "row-resize" => CursorShape::ResizeRow,
            "ew-resize" => CursorShape::ResizeEastWest,
            "ns-resize" => CursorShape::ResizeNorthSouth,
            "nesw-resize" => CursorShape::ResizeNorthEastSouthWest,
            "nwse-resize" => CursorShape::ResizeNorthWestSouthEast,
            "zoom-in" => CursorShape::ZoomIn,
            "zoom-out" => CursorShape::ZoomOut,
            _ => return None,
        })
    }
}
