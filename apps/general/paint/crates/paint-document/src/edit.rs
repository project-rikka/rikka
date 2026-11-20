use rikka_image_edit::{Intent, Profile, Recipe};
use rikka_psd::BlendMode;
use rikka_types::Color;

use crate::combine::Combine;
use crate::figure::{Figure, FigureId};
use crate::flood::Flood;
use crate::layer::LayerId;
use crate::modify::Modify;
use crate::region::Region;
use crate::reshape::Reshape;
use crate::shape::Shape;

#[derive(Clone, Debug, PartialEq)]
pub enum Edit {
    Show {
        layer: LayerId,
        visible: bool,
    },
    Select {
        region: Region,
        by: SelectionAction,
    },
    Crop(Region),
    Develop {
        layer: LayerId,
        recipe: Recipe,
    },
    Fill {
        layer: LayerId,
        color: Color,
    },
    Erase {
        layer: LayerId,
        background: Color,
    },
    Reshape(Reshape),
    Convert {
        profile: Profile,
        intent: Intent,
    },
    Undo,
    Redo,
    Revisit(usize),
    SelectShape {
        shape: Shape,
        combine: Combine,
    },
    MagicWand {
        flood: Flood,
        combine: Combine,
    },
    SelectLayer {
        layer: LayerId,
        combine: Combine,
    },
    ModifySelection(Modify),
    MoveSelection {
        dx: i32,
        dy: i32,
    },
    Bucket {
        layer: LayerId,
        flood: Flood,
        color: Color,
    },
    Figures {
        layer: LayerId,
        change: FigureEdit,
    },
    Batch {
        name: String,
        edits: Vec<Edit>,
    },
    NewLayer {
        above: LayerId,
        name: String,
    },
    DuplicateLayer {
        layer: LayerId,
        name: String,
    },
    DeleteLayer {
        layer: LayerId,
    },
    MoveLayer {
        layer: LayerId,
        to: Placement,
    },
    RenameLayer {
        layer: LayerId,
        name: String,
    },
    SetOpacity {
        layer: LayerId,
        opacity: f32,
    },
    SetBlend {
        layer: LayerId,
        blend: BlendMode,
    },
    SetProperties {
        layer: LayerId,
        properties: Properties,
    },
    MergeDown {
        layer: LayerId,
    },
    MergeVisible,
    Flatten,
    LayerFromBackground {
        name: String,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum FigureEdit {
    Add {
        figures: Vec<Figure>,
        by: AddMethod,
    },
    Change {
        figures: Vec<Figure>,
        by: FigureChange,
    },
    Remove(Vec<FigureId>),
    Stack {
        figures: Vec<FigureId>,
        to: StackOrder,
    },
    Rasterize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AddMethod {
    Draw,
    Paste,
    Duplicate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FigureChange {
    Move,
    Resize,
    Rotate,
    Restyle,
    Retext,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StackOrder {
    Front,
    Forward,
    Backward,
    Back,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Placement {
    Above(LayerId),
    Below(LayerId),
    Into(LayerId),
    Forward,
    Backward,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Properties {
    pub name: String,
    pub visible: bool,
    pub opacity: f32,
    pub blend: BlendMode,
}

impl Edit {
    #[must_use]
    pub(crate) fn travels(&self) -> bool {
        match self {
            Edit::Undo | Edit::Redo | Edit::Revisit(_) => true,
            Edit::Batch { edits, .. } => edits.iter().any(Edit::travels),
            _ => false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SelectionAction {
    Rectangle,
    All,
    Deselect,
    Invert,
    Ellipse,
    Lasso,
    Polygon,
    MagicWand,
    LayerPixels,
    Move,
    Expand,
    Contract,
    Feather,
    Border,
    Smooth,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PixelChange {
    #[default]
    Unchanged,
    Content,
    Canvas,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Change {
    pub pixels: PixelChange,
    pub selection: bool,
    pub history: bool,
    pub layers: bool,
    pub layer: Option<LayerId>,
}

impl Change {
    pub const NONE: Change = Change {
        pixels: PixelChange::Unchanged,
        selection: false,
        history: false,
        layers: false,
        layer: None,
    };

    #[must_use]
    pub fn and(self, other: Change) -> Change {
        Change {
            pixels: self.pixels.max(other.pixels),
            selection: self.selection || other.selection,
            history: self.history || other.history,
            layers: self.layers || other.layers,
            layer: other.layer.or(self.layer),
        }
    }

    #[must_use]
    pub fn is_none(self) -> bool {
        self == Change::NONE
    }
}
