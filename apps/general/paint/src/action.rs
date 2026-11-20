use paint_document::{Action, FigureAction, FigureChange, Reshape, SelectionAction, StackOrder};
use rikka_app::prelude::{Label, MaterialSymbol, tr};

use crate::operation;
use crate::tool::Tool;

pub(crate) fn name(action: &Action) -> Label {
    match action {
        Action::Open => tr!("paint.action.open"),
        Action::Show { visible: true, .. } => tr!("paint.action.show_layer"),
        Action::Show { visible: false, .. } => tr!("paint.action.hide_layer"),
        Action::Crop => tr!("paint.action.crop"),
        Action::Develop(operation) => operation::name(operation),
        Action::Fill => tr!("paint.action.fill"),
        Action::Erase => tr!("paint.action.erase"),
        Action::Reshape(reshape) => reshape_name(reshape),
        Action::Convert => tr!("paint.operation.convert_profile"),
        Action::Select(selecting) => selection_name(*selecting),
        Action::Bucket => tr!("paint.action.bucket"),
        Action::Figure(figured) => figure_name(*figured),
        Action::Batch(name) => Label::from(name.as_str()),
        Action::New => tr!("paint.action.new"),
        Action::NewLayer => tr!("paint.action.new_layer"),
        Action::DuplicateLayer => tr!("paint.action.duplicate_layer"),
        Action::DeleteLayer => tr!("paint.action.delete_layer"),
        Action::MoveLayer => tr!("paint.action.move_layer"),
        Action::RenameLayer => tr!("paint.action.rename_layer"),
        Action::LayerOpacity => tr!("paint.action.layer_opacity"),
        Action::LayerBlend => tr!("paint.action.layer_blend"),
        Action::LayerProperties => tr!("paint.action.layer_properties"),
        Action::MergeDown => tr!("paint.action.merge_down"),
        Action::MergeVisible => tr!("paint.action.merge_visible"),
        Action::Flatten => tr!("paint.action.flatten"),
        Action::LayerFromBackground => tr!("paint.action.layer_from_background"),
    }
}

fn figure_name(figured: FigureAction) -> Label {
    match figured {
        FigureAction::Draw(kind) => Tool::drawing_figure(kind).name(),
        FigureAction::Paste => tr!("paint.action.paste_objects"),
        FigureAction::Duplicate => tr!("paint.action.duplicate_objects"),
        FigureAction::Change(FigureChange::Move) => tr!("paint.action.move_objects"),
        FigureAction::Change(FigureChange::Resize) => tr!("paint.action.resize_objects"),
        FigureAction::Change(FigureChange::Rotate) => tr!("paint.action.rotate_objects"),
        FigureAction::Change(FigureChange::Restyle) => tr!("paint.action.restyle_objects"),
        FigureAction::Change(FigureChange::Retext) => tr!("paint.action.edit_text"),
        FigureAction::Delete => tr!("paint.action.delete_objects"),
        FigureAction::Stack(StackOrder::Front) => tr!("paint.command.bring_to_front"),
        FigureAction::Stack(StackOrder::Forward) => tr!("paint.command.bring_forward"),
        FigureAction::Stack(StackOrder::Backward) => tr!("paint.command.send_backward"),
        FigureAction::Stack(StackOrder::Back) => tr!("paint.command.send_to_back"),
        FigureAction::Rasterize => tr!("paint.command.rasterize_objects"),
    }
}

fn figure_symbol(figured: FigureAction) -> MaterialSymbol {
    match figured {
        FigureAction::Draw(kind) => Tool::drawing_figure(kind).symbol(),
        FigureAction::Paste => MaterialSymbol::CONTENT_PASTE,
        FigureAction::Duplicate => MaterialSymbol::CONTENT_COPY,
        FigureAction::Change(FigureChange::Move) => MaterialSymbol::OPEN_WITH,
        FigureAction::Change(FigureChange::Resize) => MaterialSymbol::PHOTO_SIZE_SELECT_LARGE,
        FigureAction::Change(FigureChange::Rotate) => MaterialSymbol::ROTATE_RIGHT,
        FigureAction::Change(FigureChange::Restyle) => MaterialSymbol::PALETTE,
        FigureAction::Change(FigureChange::Retext) => MaterialSymbol::TEXT_FIELDS,
        FigureAction::Delete => MaterialSymbol::DELETE,
        FigureAction::Stack(StackOrder::Front | StackOrder::Forward) => {
            MaterialSymbol::FLIP_TO_FRONT
        }
        FigureAction::Stack(StackOrder::Back | StackOrder::Backward) => {
            MaterialSymbol::FLIP_TO_BACK
        }
        FigureAction::Rasterize => MaterialSymbol::LAYERS,
    }
}

fn selection_name(selecting: SelectionAction) -> Label {
    match selecting {
        SelectionAction::Rectangle => tr!("paint.action.select_rectangle"),
        SelectionAction::All => tr!("paint.action.select_all"),
        SelectionAction::Deselect => tr!("paint.action.deselect"),
        SelectionAction::Invert => tr!("paint.action.invert_selection"),
        SelectionAction::Ellipse => tr!("paint.action.select_ellipse"),
        SelectionAction::Lasso => tr!("paint.action.select_lasso"),
        SelectionAction::Polygon => tr!("paint.action.select_polygon"),
        SelectionAction::MagicWand => tr!("paint.action.magic_wand"),
        SelectionAction::LayerPixels => tr!("paint.action.select_layer_pixels"),
        SelectionAction::Move => tr!("paint.action.move_selection"),
        SelectionAction::Expand => tr!("paint.action.expand_selection"),
        SelectionAction::Contract => tr!("paint.action.contract_selection"),
        SelectionAction::Feather => tr!("paint.action.feather_selection"),
        SelectionAction::Border => tr!("paint.action.border_selection"),
        SelectionAction::Smooth => tr!("paint.action.smooth_selection"),
    }
}

fn reshape_name(reshape: &Reshape) -> Label {
    match reshape {
        Reshape::Resize { .. } => tr!("paint.operation.resize"),
        Reshape::CanvasSize { .. } => tr!("paint.operation.canvas_size"),
        Reshape::Turn(turn) => operation::turn_name(*turn),
        Reshape::Flip(flip) => operation::flip_name(*flip),
        Reshape::Rotate { .. } => tr!("paint.operation.rotate"),
    }
}

pub(crate) fn symbol(action: &Action) -> MaterialSymbol {
    match action {
        Action::Open => MaterialSymbol::FOLDER_OPEN,
        Action::Show { visible: true, .. } => MaterialSymbol::VISIBILITY,
        Action::Show { visible: false, .. } => MaterialSymbol::VISIBILITY_OFF,
        Action::Select(SelectionAction::All) => MaterialSymbol::SELECT_ALL,
        Action::Select(SelectionAction::Deselect) => MaterialSymbol::DESELECT,
        Action::Select(SelectionAction::Rectangle | SelectionAction::Invert) => {
            MaterialSymbol::HIGHLIGHT_ALT
        }
        Action::Crop => MaterialSymbol::CROP,
        Action::Develop(operation) => operation::symbol(operation),
        Action::Fill => MaterialSymbol::FORMAT_COLOR_FILL,
        Action::Erase => MaterialSymbol::INK_ERASER,
        Action::Reshape(Reshape::Resize { .. }) => MaterialSymbol::PHOTO_SIZE_SELECT_LARGE,
        Action::Reshape(Reshape::CanvasSize { .. }) => MaterialSymbol::ASPECT_RATIO,
        Action::Reshape(Reshape::Turn(_) | Reshape::Rotate { .. }) => MaterialSymbol::ROTATE_RIGHT,
        Action::Reshape(Reshape::Flip(_)) => MaterialSymbol::FLIP,
        Action::Convert => MaterialSymbol::COLOR_LENS,
        Action::Select(SelectionAction::Ellipse) => MaterialSymbol::BLUR_CIRCULAR,
        Action::Select(SelectionAction::Lasso) => MaterialSymbol::LASSO_SELECT,
        Action::Select(SelectionAction::Polygon) => MaterialSymbol::PENTAGON,
        Action::Select(SelectionAction::MagicWand) => MaterialSymbol::AUTO_FIX_HIGH,
        Action::Select(SelectionAction::LayerPixels) => MaterialSymbol::LAYERS,
        Action::Select(SelectionAction::Move) => MaterialSymbol::OPEN_WITH,
        Action::Select(SelectionAction::Expand) => MaterialSymbol::ZOOM_OUT_MAP,
        Action::Select(SelectionAction::Contract) => MaterialSymbol::ZOOM_IN_MAP,
        Action::Select(SelectionAction::Feather) => MaterialSymbol::BLUR_ON,
        Action::Select(SelectionAction::Border) => MaterialSymbol::BORDER_OUTER,
        Action::Select(SelectionAction::Smooth) => MaterialSymbol::LINE_CURVE,
        Action::Bucket => MaterialSymbol::FORMAT_COLOR_FILL,
        Action::Figure(figured) => figure_symbol(*figured),
        Action::Batch(_) => MaterialSymbol::AUTO_AWESOME,
        Action::New => MaterialSymbol::NOTE_ADD,
        Action::NewLayer => MaterialSymbol::ADD,
        Action::DuplicateLayer => MaterialSymbol::CONTENT_COPY,
        Action::DeleteLayer => MaterialSymbol::DELETE,
        Action::MoveLayer => MaterialSymbol::DRAG_INDICATOR,
        Action::RenameLayer => MaterialSymbol::DRIVE_FILE_RENAME_OUTLINE,
        Action::LayerOpacity => MaterialSymbol::OPACITY,
        Action::LayerBlend | Action::LayerProperties => MaterialSymbol::TUNE,
        Action::MergeDown | Action::MergeVisible => MaterialSymbol::CALL_MERGE,
        Action::Flatten => MaterialSymbol::LAYERS_CLEAR,
        Action::LayerFromBackground => MaterialSymbol::LOCK_OPEN,
    }
}
