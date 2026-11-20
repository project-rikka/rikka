use std::sync::LazyLock;

use paint_document::{Reshape, StackOrder};
use rikka_app::prelude::*;
use rikka_image_edit::{Desaturate, Flip, QuarterTurn};
use rikka_input::{ActionMap, Chord};
use rikka_ui::Command as Standard;

use crate::direct::Direct;
use crate::paint::Paint;
use crate::session::{Kind, Modification};
use crate::tab::Layering;
use crate::tool::Tool;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Command {
    Open,
    Save,
    SaveAs,
    CloseTab,
    Quit,
    NextTab,
    PreviousTab,
    ZoomIn,
    ZoomOut,
    Fit,
    ActualSize,
    Undo,
    Redo,
    SelectAll,
    Deselect,
    InvertSelection,
    EraseSelection,
    CropToSelection,
    Turn(QuarterTurn),
    Flip(Flip),
    Begin(Kind),
    Apply,
    Cancel,
    Pick(Tool),
    NextShape,
    FillSelection,
    SwapColors,
    ResetColors,
    Thinner,
    Thicker,
    Develop(Direct),
    RepeatEffect,
    ReopenEffect,
    NextSelection,
    Nudge { dx: i32, dy: i32 },
    SelectLayerPixels,
    Assistant,
    NewImage,
    Layer(Layering),
    Clear,
    Object(Objecting),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Objecting {
    Cut,
    Copy,
    Paste,
    Duplicate,
    Delete,
    Stack(StackOrder),
    Rasterize,
}

const CONTROL_SHIFT: Modifiers = Modifiers {
    shift: true,
    control: true,
    ..Modifiers::NONE
};

const BINDINGS: [(Modifiers, Key, Command); 78] = [
    (Modifiers::CONTROL, Key::Char('o'), Command::Open),
    (Modifiers::CONTROL, Key::Char('s'), Command::Save),
    (CONTROL_SHIFT, Key::Char('s'), Command::SaveAs),
    (Modifiers::CONTROL, Key::Char('w'), Command::CloseTab),
    (Modifiers::CONTROL, Key::Char('q'), Command::Quit),
    (Modifiers::CONTROL, Key::Tab, Command::NextTab),
    (CONTROL_SHIFT, Key::Tab, Command::PreviousTab),
    (Modifiers::CONTROL, Key::PageDown, Command::NextTab),
    (Modifiers::CONTROL, Key::PageUp, Command::PreviousTab),
    (Modifiers::CONTROL, Key::Char('+'), Command::ZoomIn),
    (Modifiers::CONTROL, Key::Char('='), Command::ZoomIn),
    (CONTROL_SHIFT, Key::Char('+'), Command::ZoomIn),
    (Modifiers::CONTROL, Key::Char('-'), Command::ZoomOut),
    (Modifiers::CONTROL, Key::Char('0'), Command::Fit),
    (Modifiers::CONTROL, Key::Char('1'), Command::ActualSize),
    (Modifiers::CONTROL, Key::Char('z'), Command::Undo),
    (CONTROL_SHIFT, Key::Char('z'), Command::Redo),
    (Modifiers::CONTROL, Key::Char('y'), Command::Redo),
    (Modifiers::CONTROL, Key::Char('a'), Command::SelectAll),
    (Modifiers::CONTROL, Key::Char('d'), Command::Deselect),
    (Modifiers::CONTROL, Key::Char('i'), Command::InvertSelection),
    (Modifiers::NONE, Key::Delete, Command::Clear),
    (
        Modifiers::CONTROL,
        Key::Char('x'),
        Command::Object(Objecting::Cut),
    ),
    (
        Modifiers::CONTROL,
        Key::Char('c'),
        Command::Object(Objecting::Copy),
    ),
    (
        Modifiers::CONTROL,
        Key::Char('v'),
        Command::Object(Objecting::Paste),
    ),
    (
        CONTROL_SHIFT,
        Key::Char(']'),
        Command::Object(Objecting::Stack(StackOrder::Front)),
    ),
    (
        CONTROL_SHIFT,
        Key::Char('}'),
        Command::Object(Objecting::Stack(StackOrder::Front)),
    ),
    (
        CONTROL_SHIFT,
        Key::Char('['),
        Command::Object(Objecting::Stack(StackOrder::Back)),
    ),
    (
        CONTROL_SHIFT,
        Key::Char('{'),
        Command::Object(Objecting::Stack(StackOrder::Back)),
    ),
    (
        Modifiers::NONE,
        Key::Char('v'),
        Command::Pick(Tool::Objects),
    ),
    (CONTROL_SHIFT, Key::Char('x'), Command::CropToSelection),
    (
        Modifiers::CONTROL,
        Key::Char('r'),
        Command::Begin(Kind::Resize),
    ),
    (
        CONTROL_SHIFT,
        Key::Char('r'),
        Command::Begin(Kind::CanvasSize),
    ),
    (
        Modifiers::CONTROL,
        Key::Char('h'),
        Command::Turn(QuarterTurn::Clockwise),
    ),
    (
        Modifiers::CONTROL,
        Key::Char('g'),
        Command::Turn(QuarterTurn::CounterClockwise),
    ),
    (
        Modifiers::CONTROL,
        Key::Char('j'),
        Command::Turn(QuarterTurn::Half),
    ),
    (Modifiers::NONE, Key::Enter, Command::Apply),
    (Modifiers::NONE, Key::Escape, Command::Cancel),
    (Modifiers::NONE, Key::Char('s'), Command::NextSelection),
    (Modifiers::NONE, Key::Char('h'), Command::Pick(Tool::Hand)),
    (Modifiers::NONE, Key::Char('z'), Command::Pick(Tool::Zoom)),
    (Modifiers::NONE, Key::Char('p'), Command::Pick(Tool::Pen)),
    (
        Modifiers::SHIFT,
        Key::Char('b'),
        Command::Pick(Tool::Highlighter),
    ),
    (Modifiers::NONE, Key::Char('l'), Command::Pick(Tool::Line)),
    (Modifiers::NONE, Key::Char('a'), Command::Pick(Tool::Arrow)),
    (Modifiers::NONE, Key::Char('u'), Command::NextShape),
    (
        Modifiers::NONE,
        Key::Char('i'),
        Command::Pick(Tool::ColorPicker),
    ),
    (
        Modifiers::NONE,
        Key::Char('k'),
        Command::Pick(Tool::ColorPicker),
    ),
    (Modifiers::NONE, Key::Backspace, Command::FillSelection),
    (Modifiers::NONE, Key::Char('x'), Command::SwapColors),
    (Modifiers::NONE, Key::Char('d'), Command::ResetColors),
    (Modifiers::NONE, Key::Char('['), Command::Thinner),
    (Modifiers::NONE, Key::Char(']'), Command::Thicker),
    (
        Modifiers::NONE,
        Key::Char('b'),
        Command::Pick(Tool::Paintbrush),
    ),
    (Modifiers::NONE, Key::Char('e'), Command::Pick(Tool::Eraser)),
    (
        Modifiers::NONE,
        Key::Char('g'),
        Command::Pick(Tool::Gradient),
    ),
    (
        Modifiers::NONE,
        Key::Char('o'),
        Command::Pick(Tool::FreeformShape),
    ),
    (
        Modifiers::NONE,
        Key::Char('w'),
        Command::Pick(Tool::MagicWand),
    ),
    (
        Modifiers::NONE,
        Key::Char('f'),
        Command::Pick(Tool::PaintBucket),
    ),
    (
        Modifiers::NONE,
        Key::ArrowLeft,
        Command::Nudge { dx: -1, dy: 0 },
    ),
    (
        Modifiers::NONE,
        Key::ArrowRight,
        Command::Nudge { dx: 1, dy: 0 },
    ),
    (
        Modifiers::NONE,
        Key::ArrowUp,
        Command::Nudge { dx: 0, dy: -1 },
    ),
    (
        Modifiers::NONE,
        Key::ArrowDown,
        Command::Nudge { dx: 0, dy: 1 },
    ),
    (
        Modifiers::SHIFT,
        Key::ArrowLeft,
        Command::Nudge { dx: -10, dy: 0 },
    ),
    (
        Modifiers::SHIFT,
        Key::ArrowRight,
        Command::Nudge { dx: 10, dy: 0 },
    ),
    (
        Modifiers::SHIFT,
        Key::ArrowUp,
        Command::Nudge { dx: 0, dy: -10 },
    ),
    (
        Modifiers::SHIFT,
        Key::ArrowDown,
        Command::Nudge { dx: 0, dy: 10 },
    ),
    (
        Modifiers::SHIFT,
        Key::F(6),
        Command::Begin(Kind::Modify(Modification::Feather)),
    ),
    (Modifiers::NONE, Key::Char('t'), Command::Pick(Tool::Text)),
    (Modifiers::CONTROL, Key::Char('n'), Command::NewImage),
    (CONTROL_SHIFT, Key::Char('n'), Command::Layer(Layering::New)),
    (
        CONTROL_SHIFT,
        Key::Char('d'),
        Command::Layer(Layering::Duplicate),
    ),
    (CONTROL_SHIFT, Key::Delete, Command::Layer(Layering::Delete)),
    (
        Modifiers::CONTROL,
        Key::Char(']'),
        Command::Layer(Layering::Forward),
    ),
    (
        Modifiers::CONTROL,
        Key::Char('['),
        Command::Layer(Layering::Backward),
    ),
    (
        Modifiers::CONTROL,
        Key::Char('e'),
        Command::Layer(Layering::MergeDown),
    ),
    (
        CONTROL_SHIFT,
        Key::Char('f'),
        Command::Layer(Layering::Flatten),
    ),
    (
        Modifiers::NONE,
        Key::F(4),
        Command::Begin(Kind::LayerProperties),
    ),
];

const CONTROL_ALT: Modifiers = Modifiers {
    alt: true,
    control: true,
    ..Modifiers::NONE
};

const CONTROL_ALT_SHIFT: Modifiers = Modifiers {
    alt: true,
    control: true,
    shift: true,
    ..Modifiers::NONE
};

const DEVELOPING: [(Modifiers, Key, Command); 14] = [
    (
        CONTROL_SHIFT,
        Key::Char('l'),
        Command::Develop(Direct::AutoLevels),
    ),
    (
        CONTROL_ALT_SHIFT,
        Key::Char('l'),
        Command::Develop(Direct::AutoContrast),
    ),
    (
        CONTROL_SHIFT,
        Key::Char('b'),
        Command::Develop(Direct::AutoColor),
    ),
    (
        Modifiers::CONTROL,
        Key::Char('l'),
        Command::Begin(Kind::Develop(rikka_effects::Kind::Levels)),
    ),
    (
        Modifiers::CONTROL,
        Key::Char('m'),
        Command::Begin(Kind::Develop(rikka_effects::Kind::Curves)),
    ),
    (
        Modifiers::CONTROL,
        Key::Char('u'),
        Command::Begin(Kind::Develop(rikka_effects::Kind::HueSaturation)),
    ),
    (
        Modifiers::CONTROL,
        Key::Char('b'),
        Command::Begin(Kind::Develop(rikka_effects::Kind::ColorBalance)),
    ),
    (
        CONTROL_ALT_SHIFT,
        Key::Char('b'),
        Command::Begin(Kind::Develop(rikka_effects::Kind::BlackWhite)),
    ),
    (
        CONTROL_SHIFT,
        Key::Char('i'),
        Command::Develop(Direct::Invert),
    ),
    (
        CONTROL_SHIFT,
        Key::Char('p'),
        Command::Begin(Kind::Develop(rikka_effects::Kind::Posterize)),
    ),
    (
        CONTROL_SHIFT,
        Key::Char('u'),
        Command::Develop(Direct::Desaturate(Desaturate::Lightness)),
    ),
    (
        CONTROL_SHIFT,
        Key::Char('e'),
        Command::Develop(Direct::Sepia),
    ),
    (Modifiers::CONTROL, Key::Char('f'), Command::RepeatEffect),
    (CONTROL_ALT, Key::Char('f'), Command::ReopenEffect),
];

const ASSISTING: [(Modifiers, Key, Command); 1] =
    [(Modifiers::CONTROL, Key::Char('k'), Command::Assistant)];

static KEYMAP: LazyLock<ActionMap<Command>> = LazyLock::new(|| {
    BINDINGS
        .into_iter()
        .chain(DEVELOPING)
        .chain(ASSISTING)
        .fold(ActionMap::new(), |map, (modifiers, key, command)| {
            map.bind(Chord::new(modifiers, key), command)
        })
});

impl Command {
    pub(crate) fn pressed(modifiers: Modifiers, key: Key) -> Option<Command> {
        KEYMAP.resolve(modifiers, key).copied()
    }

    pub(crate) fn from_standard(standard: Standard) -> Option<Command> {
        Some(match standard {
            Standard::Undo => Command::Undo,
            Standard::Redo => Command::Redo,
            Standard::Cut => Command::Object(Objecting::Cut),
            Standard::Copy => Command::Object(Objecting::Copy),
            Standard::Paste => Command::Object(Objecting::Paste),
            Standard::SelectAll => Command::SelectAll,
            Standard::SelectNone => Command::Deselect,
            _ => return None,
        })
    }

    pub(crate) fn chord(self) -> Option<Chord> {
        match self {
            Command::Pick(Tool::Rectangle | Tool::Ellipse) => KEYMAP.chord_for(&Command::NextShape),
            Command::Pick(
                Tool::RectangleSelect | Tool::EllipseSelect | Tool::Lasso | Tool::PolygonLasso,
            ) => KEYMAP.chord_for(&Command::NextSelection),
            Command::EraseSelection | Command::Object(Objecting::Delete) => {
                KEYMAP.chord_for(&Command::Clear)
            }
            _ => KEYMAP.chord_for(&self),
        }
    }

    pub(crate) fn run(self, paint: &mut Paint) {
        match self {
            Command::Open => paint.show_open_dialog(),
            Command::Save => paint.save(),
            Command::SaveAs => paint.save_as(),
            Command::CloseTab => paint.close_active(),
            Command::Quit => paint.ask_to_close_window(),
            Command::NextTab => paint.step_tab(true),
            Command::PreviousTab => paint.step_tab(false),
            Command::ZoomIn => paint.zoom(true),
            Command::ZoomOut => paint.zoom(false),
            Command::Fit => paint.fit(),
            Command::ActualSize => paint.actual_size(),
            Command::Undo => paint.undo(),
            Command::Redo => paint.redo(),
            Command::SelectAll if paint.tool() == Tool::Objects => paint.pick_all_figures(),
            Command::SelectAll => paint.select_all(),
            Command::Deselect => paint.deselect(),
            Command::InvertSelection => paint.invert_selection(),
            Command::EraseSelection => paint.erase_selection(),
            Command::CropToSelection => paint.crop_to_selection(),
            Command::Turn(turn) => paint.reshape(Reshape::Turn(turn)),
            Command::Flip(flip) => paint.reshape(Reshape::Flip(flip)),
            Command::Begin(kind) => paint.begin(kind),
            Command::Apply => paint.apply_session(),
            Command::Cancel => match (paint.creating().is_some(), paint.unsaved_prompt().is_some())
            {
                (true, _) => paint.cancel_new_image(),
                (false, true) => paint.answer(Unsaved::Cancel),
                (false, false) if paint.in_session() => paint.cancel_session(),
                (false, false) => paint.unpick(),
            },
            Command::Pick(tool) => paint.pick(tool),
            Command::NextShape => paint.cycle_shapes(),
            Command::FillSelection => paint.fill_selection(),
            Command::SwapColors => paint.swap_colors(),
            Command::ResetColors => paint.reset_colors(),
            Command::Thinner => paint.step_width(false),
            Command::Thicker => paint.step_width(true),
            Command::Develop(direct) => paint.develop_directly(direct),
            Command::RepeatEffect => paint.repeat_effect(),
            Command::ReopenEffect => paint.reopen_effect(),
            Command::NextSelection => paint.cycle_selections(),
            Command::Nudge { dx, dy } if paint.tool().picks() && paint.has_picks() => {
                paint.nudge_figures((dx, dy));
            }
            Command::Nudge { dx, dy } => paint.move_selection((dx, dy)),
            Command::SelectLayerPixels => paint.select_layer_pixels(),
            Command::Assistant => paint.open_assistant(),
            Command::NewImage => paint.begin_new_image(),
            Command::Layer(order) => paint.arrange(order),
            Command::Clear if paint.has_picks() => paint.delete_figures(),
            Command::Clear => paint.erase_selection(),
            Command::Object(objecting) => match objecting {
                Objecting::Cut => paint.cut_figures(),
                Objecting::Copy => paint.copy_figures(),
                Objecting::Paste => paint.paste_figures(),
                Objecting::Duplicate => paint.duplicate_figures(),
                Objecting::Delete => paint.delete_figures(),
                Objecting::Stack(to) => paint.stack_figures(to),
                Objecting::Rasterize => paint.rasterize_figures(),
            },
        }
    }

    pub(crate) fn enabled(self, paint: &Paint) -> bool {
        if paint.creating().is_some() {
            return self == Command::Cancel;
        }
        let unsaved_prompt = paint.unsaved_prompt().is_some();
        match self {
            Command::Open
            | Command::Pick(_)
            | Command::NextShape
            | Command::SwapColors
            | Command::ResetColors => !unsaved_prompt,
            Command::Thinner | Command::Thicker => !unsaved_prompt && paint.draws(),
            Command::Quit => !unsaved_prompt,
            Command::Save | Command::SaveAs => paint.can_save(),
            Command::CloseTab => !unsaved_prompt && paint.active_tab().is_some(),
            Command::NextTab | Command::PreviousTab => !unsaved_prompt && paint.tab_count() > 1,
            Command::ZoomIn => paint.can_zoom(true),
            Command::ZoomOut => paint.can_zoom(false),
            Command::Fit | Command::ActualSize => paint.shows_picture(),
            Command::Undo => paint.can_undo(),
            Command::Redo => paint.can_redo(),
            Command::SelectAll if paint.tool() == Tool::Objects => paint.can_pick_all(),
            Command::SelectAll | Command::InvertSelection => paint.takes_selection(),
            Command::Deselect | Command::CropToSelection => paint.has_selection(),
            Command::EraseSelection | Command::FillSelection => paint.can_erase(),
            Command::Begin(kind) if kind.develops() => paint.develops(),
            Command::Begin(Kind::Modify(_)) => paint.has_selection(),
            Command::Begin(Kind::LayerProperties) => paint.can_restyle(),
            Command::Turn(_) | Command::Flip(_) | Command::Begin(_) => paint.edits(),
            Command::Develop(_) => paint.develops(),
            Command::RepeatEffect | Command::ReopenEffect => {
                paint.develops() && paint.last_effect().is_some()
            }
            Command::Apply => paint.in_session(),
            Command::Cancel => unsaved_prompt || paint.in_session() || paint.has_picks(),
            Command::NextSelection => !unsaved_prompt,
            Command::Nudge { .. } => {
                (paint.has_selection() && paint.tool().selects())
                    || (paint.has_picks() && paint.tool().picks())
            }
            Command::SelectLayerPixels => paint.selects_layer(),
            Command::Assistant => true,
            Command::NewImage => !unsaved_prompt,
            Command::Layer(order) => paint.can_arrange(order),
            Command::Clear => paint.has_picks() || paint.can_erase(),
            Command::Object(Objecting::Paste) => paint.can_paste_figures(),
            Command::Object(Objecting::Rasterize) => paint.can_rasterize(),
            Command::Object(_) => paint.has_picks(),
        }
    }

    pub(crate) fn tooltip(self, label: Label) -> Tooltip {
        Tooltip::new(label).shortcut(self.chord())
    }

    pub(crate) fn entry(self, label: Label, paint: &Paint) -> MenuItem {
        let item = menu_item(label)
            .enabled(self.enabled(paint))
            .on_press(move |paint: &mut Paint| self.run(paint));
        match self.chord() {
            Some(chord) => item.shortcut(chord.to_string()),
            None => item,
        }
    }
}
