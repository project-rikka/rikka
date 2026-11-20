use paint_document::FigureKind;
use rikka_app::prelude::{Label, MaterialSymbol, tr};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Tool {
    RectangleSelect,
    #[default]
    Hand,
    Zoom,
    Pen,
    Highlighter,
    Line,
    Arrow,
    Rectangle,
    Ellipse,
    ColorPicker,
    EllipseSelect,
    Lasso,
    PolygonLasso,
    MagicWand,
    PaintBucket,
    Text,
    Paintbrush,
    Eraser,
    Gradient,
    FreeformShape,
    Objects,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Family {
    Arranging,
    Navigation,
    Drawing,
    Shapes,
    Picking,
    SelectionAction,
    Lettering,
}

impl Tool {
    pub(crate) const ALL: [Tool; 21] = [
        Tool::Objects,
        Tool::RectangleSelect,
        Tool::EllipseSelect,
        Tool::Lasso,
        Tool::PolygonLasso,
        Tool::MagicWand,
        Tool::Hand,
        Tool::Zoom,
        Tool::Pen,
        Tool::Highlighter,
        Tool::Paintbrush,
        Tool::Eraser,
        Tool::PaintBucket,
        Tool::Gradient,
        Tool::Line,
        Tool::Arrow,
        Tool::Rectangle,
        Tool::Ellipse,
        Tool::FreeformShape,
        Tool::ColorPicker,
        Tool::Text,
    ];

    pub(crate) fn symbol(self) -> MaterialSymbol {
        match self {
            Tool::RectangleSelect => MaterialSymbol::HIGHLIGHT_ALT,
            Tool::Hand => MaterialSymbol::PAN_TOOL,
            Tool::Zoom => MaterialSymbol::ZOOM_IN,
            Tool::Pen => MaterialSymbol::DRAW,
            Tool::Highlighter => MaterialSymbol::INK_HIGHLIGHTER,
            Tool::Line => MaterialSymbol::PEN_SIZE_2,
            Tool::Arrow => MaterialSymbol::NORTH_EAST,
            Tool::Rectangle => MaterialSymbol::CROP_SQUARE,
            Tool::Ellipse => MaterialSymbol::PANORAMA_FISH_EYE,
            Tool::ColorPicker => MaterialSymbol::COLORIZE,
            Tool::EllipseSelect => MaterialSymbol::BLUR_CIRCULAR,
            Tool::Lasso => MaterialSymbol::LASSO_SELECT,
            Tool::PolygonLasso => MaterialSymbol::PENTAGON,
            Tool::MagicWand => MaterialSymbol::AUTO_FIX_HIGH,
            Tool::PaintBucket => MaterialSymbol::FORMAT_COLOR_FILL,
            Tool::Text => MaterialSymbol::TEXT_FIELDS,
            Tool::Paintbrush => MaterialSymbol::BRUSH,
            Tool::Eraser => MaterialSymbol::INK_ERASER,
            Tool::Gradient => MaterialSymbol::GRADIENT,
            Tool::FreeformShape => MaterialSymbol::GESTURE,
            Tool::Objects => MaterialSymbol::ARROW_SELECTOR_TOOL,
        }
    }

    pub(crate) fn name(self) -> Label {
        match self {
            Tool::RectangleSelect => tr!("paint.tool.rectangle_select"),
            Tool::Hand => tr!("paint.tool.hand"),
            Tool::Zoom => tr!("paint.tool.zoom"),
            Tool::Pen => tr!("paint.tool.pen"),
            Tool::Highlighter => tr!("paint.tool.highlighter"),
            Tool::Line => tr!("paint.tool.line"),
            Tool::Arrow => tr!("paint.tool.arrow"),
            Tool::Rectangle => tr!("paint.tool.rectangle"),
            Tool::Ellipse => tr!("paint.tool.ellipse"),
            Tool::ColorPicker => tr!("paint.tool.color_picker"),
            Tool::EllipseSelect => tr!("paint.tool.ellipse_select"),
            Tool::Lasso => tr!("paint.tool.lasso"),
            Tool::PolygonLasso => tr!("paint.tool.polygon_lasso"),
            Tool::MagicWand => tr!("paint.tool.magic_wand"),
            Tool::PaintBucket => tr!("paint.tool.paint_bucket"),
            Tool::Text => tr!("paint.tool.text"),
            Tool::Paintbrush => tr!("paint.tool.paintbrush"),
            Tool::Eraser => tr!("paint.tool.eraser"),
            Tool::Gradient => tr!("paint.tool.gradient"),
            Tool::FreeformShape => tr!("paint.tool.freeform_shape"),
            Tool::Objects => tr!("paint.tool.objects"),
        }
    }

    pub(crate) fn family(self) -> Family {
        match self {
            Tool::Objects => Family::Arranging,
            Tool::Hand | Tool::Zoom => Family::Navigation,
            Tool::Pen
            | Tool::Highlighter
            | Tool::Paintbrush
            | Tool::Eraser
            | Tool::PaintBucket
            | Tool::Gradient => Family::Drawing,
            Tool::Line | Tool::Arrow | Tool::Rectangle | Tool::Ellipse | Tool::FreeformShape => {
                Family::Shapes
            }
            Tool::ColorPicker => Family::Picking,
            Tool::RectangleSelect
            | Tool::EllipseSelect
            | Tool::Lasso
            | Tool::PolygonLasso
            | Tool::MagicWand => Family::SelectionAction,
            Tool::Text => Family::Lettering,
        }
    }

    pub(crate) fn paints(self) -> bool {
        self.sized() || matches!(self, Tool::Gradient | Tool::PaintBucket | Tool::Text)
    }

    pub(crate) fn sized(self) -> bool {
        self.marks().is_some()
            || self.shape().is_some()
            || matches!(self, Tool::Paintbrush | Tool::Eraser)
    }

    pub(crate) fn marks(self) -> Option<rikka_image_edit::Tool> {
        match self {
            Tool::Pen => Some(rikka_image_edit::Tool::Pen),
            Tool::Highlighter => Some(rikka_image_edit::Tool::Highlighter),
            _ => None,
        }
    }

    pub(crate) fn shape(self) -> Option<FigureKind> {
        match self {
            Tool::Line => Some(FigureKind::Line),
            Tool::Arrow => Some(FigureKind::Arrow),
            Tool::Rectangle => Some(FigureKind::Rectangle),
            Tool::Ellipse => Some(FigureKind::Ellipse),
            Tool::FreeformShape => Some(FigureKind::Path),
            _ => None,
        }
    }

    pub(crate) fn fills(self) -> bool {
        matches!(
            self.shape(),
            Some(FigureKind::Rectangle | FigureKind::Ellipse | FigureKind::Path)
        )
    }

    pub(crate) fn picks(self) -> bool {
        self == Tool::Objects || self.shape().is_some()
    }

    pub(crate) fn drawing_figure(kind: FigureKind) -> Tool {
        match kind {
            FigureKind::Line => Tool::Line,
            FigureKind::Arrow => Tool::Arrow,
            FigureKind::Rectangle => Tool::Rectangle,
            FigureKind::Ellipse => Tool::Ellipse,
            FigureKind::Path => Tool::FreeformShape,
            FigureKind::Text => Tool::Text,
        }
    }

    pub(crate) fn drawing(marks: rikka_image_edit::Tool) -> Tool {
        match marks {
            rikka_image_edit::Tool::Pen => Tool::Pen,
            rikka_image_edit::Tool::Highlighter => Tool::Highlighter,
            rikka_image_edit::Tool::Line => Tool::Line,
            rikka_image_edit::Tool::Arrow => Tool::Arrow,
            rikka_image_edit::Tool::Rectangle => Tool::Rectangle,
            rikka_image_edit::Tool::Ellipse => Tool::Ellipse,
        }
    }

    pub(crate) fn next_shape(self) -> Tool {
        match self {
            Tool::Rectangle => Tool::Ellipse,
            _ => Tool::Rectangle,
        }
    }

    pub(crate) fn next_selection(self) -> Tool {
        match self {
            Tool::RectangleSelect => Tool::EllipseSelect,
            Tool::EllipseSelect => Tool::Lasso,
            Tool::Lasso => Tool::PolygonLasso,
            _ => Tool::RectangleSelect,
        }
    }

    pub(crate) fn selects(self) -> bool {
        self.family() == Family::SelectionAction
    }
}

pub(crate) fn drawn(marks: &rikka_image_edit::Marks) -> Option<Tool> {
    marks.marks().first().map(|marking| match marking {
        rikka_image_edit::Marking::Mark(mark) => Tool::drawing(mark.tool()),
        rikka_image_edit::Marking::Brush(stroke) if stroke.erases() => Tool::Eraser,
        rikka_image_edit::Marking::Brush(_) => Tool::Paintbrush,
        rikka_image_edit::Marking::Path(_) => Tool::FreeformShape,
    })
}
