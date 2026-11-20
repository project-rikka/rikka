use std::sync::LazyLock;

use cut_project::Point;
use rikka_app::prelude::*;
use rikka_effects::Kind;
use rikka_input::{ActionMap, Chord};
use rikka_ui::Command as Standard;

use crate::cut::{Cut, EditDirection};
use crate::quality::Quality;
use crate::tool::Tool;
use crate::transport::Direction;
use crate::trimmer::Trimmer;
use crate::wording;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Command {
    New,
    Open,
    Import,
    Save,
    SaveAs,
    Export,
    ExportFrame,
    Quit,
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    PasteAttributes,
    PasteEffects,
    RemoveEffects,
    RelinkMedia,
    ReplaceMedia,
    SelectAll,
    Deselect,
    Delete,
    RippleDelete,
    Split,
    Snapping,
    PlayPause,
    Stop,
    Pause,
    Shuttle(Direction),
    Step(i64),
    PreviousEdit,
    NextEdit,
    GoToStart,
    GoToEnd,
    MarkIn,
    MarkOut,
    ClearMarks,
    GoToIn,
    GoToOut,
    ZoomIn,
    ZoomOut,
    ZoomToFit,
    Arm(Tool),
    Add(Tool),
    AddRange,
    AddWhole,
    AddCamera,
    AddEffect(Kind),
    PreviousKey,
    NextKey,
    KeyLanes,
    GraphEditor,
    Interpolate(cut_project::Interpolation),
    Quality(Quality),
    Thumbnails(cut_filmstrip::Mode),
    Waveform(cut_waveform::Scale),
    LayerHeight(cut_project::Height),
    Trimmer(Trimmer),
    RippleTrimTo(EditDirection),
    TrimTo(EditDirection),
    ExtendEdit,
    AddMarker,
    AddClipMarker,
    PreviousMarker,
    NextMarker,
    EditMarker,
    DeleteMarker,
    ClearMarkers,
    ProjectSettings,
}

const CONTROL_SHIFT: Modifiers = Modifiers {
    shift: true,
    control: true,
    ..Modifiers::NONE
};

const CONTROL_ALT: Modifiers = Modifiers {
    alt: true,
    control: true,
    ..Modifiers::NONE
};

const BINDINGS: [(Modifiers, Key, Command); 69] = [
    (Modifiers::CONTROL, Key::Char('n'), Command::New),
    (Modifiers::CONTROL, Key::Char('o'), Command::Open),
    (Modifiers::CONTROL, Key::Char('i'), Command::Import),
    (Modifiers::CONTROL, Key::Char('s'), Command::Save),
    (CONTROL_SHIFT, Key::Char('s'), Command::SaveAs),
    (Modifiers::CONTROL, Key::Char('m'), Command::Export),
    (CONTROL_SHIFT, Key::Char('e'), Command::ExportFrame),
    (Modifiers::CONTROL, Key::Char('q'), Command::Quit),
    (Modifiers::CONTROL, Key::Char('z'), Command::Undo),
    (Modifiers::CONTROL, Key::Char('y'), Command::Redo),
    (CONTROL_SHIFT, Key::Char('z'), Command::Redo),
    (Modifiers::CONTROL, Key::Char('x'), Command::Cut),
    (Modifiers::CONTROL, Key::Char('c'), Command::Copy),
    (Modifiers::CONTROL, Key::Char('v'), Command::Paste),
    (CONTROL_ALT, Key::Char('v'), Command::PasteAttributes),
    (Modifiers::CONTROL, Key::Char('a'), Command::SelectAll),
    (Modifiers::NONE, Key::Escape, Command::Deselect),
    (CONTROL_SHIFT, Key::Char('a'), Command::Deselect),
    (Modifiers::NONE, Key::Delete, Command::Delete),
    (Modifiers::NONE, Key::Backspace, Command::Delete),
    (Modifiers::SHIFT, Key::Delete, Command::RippleDelete),
    (Modifiers::SHIFT, Key::Backspace, Command::RippleDelete),
    (Modifiers::CONTROL, Key::Char('b'), Command::Split),
    (Modifiers::NONE, Key::Char('s'), Command::Snapping),
    (Modifiers::NONE, Key::Space, Command::PlayPause),
    (
        Modifiers::NONE,
        Key::Char('l'),
        Command::Shuttle(Direction::Forward),
    ),
    (
        Modifiers::NONE,
        Key::Char('j'),
        Command::Shuttle(Direction::Backward),
    ),
    (Modifiers::NONE, Key::Char('k'), Command::Pause),
    (Modifiers::NONE, Key::ArrowLeft, Command::Step(-1)),
    (Modifiers::NONE, Key::ArrowRight, Command::Step(1)),
    (Modifiers::SHIFT, Key::ArrowLeft, Command::Step(-5)),
    (Modifiers::SHIFT, Key::ArrowRight, Command::Step(5)),
    (Modifiers::CONTROL, Key::ArrowLeft, Command::PreviousEdit),
    (Modifiers::CONTROL, Key::ArrowRight, Command::NextEdit),
    (Modifiers::NONE, Key::Home, Command::GoToStart),
    (Modifiers::NONE, Key::End, Command::GoToEnd),
    (Modifiers::NONE, Key::Char('i'), Command::MarkIn),
    (Modifiers::NONE, Key::Char('o'), Command::MarkOut),
    (CONTROL_SHIFT, Key::Char('x'), Command::ClearMarks),
    (Modifiers::SHIFT, Key::Char('i'), Command::GoToIn),
    (Modifiers::SHIFT, Key::Char('o'), Command::GoToOut),
    (Modifiers::NONE, Key::Char('='), Command::ZoomIn),
    (Modifiers::NONE, Key::Char('+'), Command::ZoomIn),
    (Modifiers::SHIFT, Key::Char('+'), Command::ZoomIn),
    (Modifiers::NONE, Key::Char('-'), Command::ZoomOut),
    (Modifiers::NONE, Key::Char('\\'), Command::ZoomToFit),
    (Modifiers::NONE, Key::Char('.'), Command::Stop),
    (Modifiers::NONE, Key::Char('v'), Command::Arm(Tool::Select)),
    (Modifiers::NONE, Key::Char('y'), Command::Arm(Tool::Anchor)),
    (Modifiers::NONE, Key::Char('t'), Command::Arm(Tool::Text)),
    (
        Modifiers::NONE,
        Key::Char('b'),
        Command::Trimmer(Trimmer::Ripple),
    ),
    (
        Modifiers::NONE,
        Key::Char('n'),
        Command::Trimmer(Trimmer::Roll),
    ),
    (
        Modifiers::SHIFT,
        Key::Char('y'),
        Command::Trimmer(Trimmer::Slip),
    ),
    (
        Modifiers::SHIFT,
        Key::Char('u'),
        Command::Trimmer(Trimmer::Slide),
    ),
    (
        Modifiers::NONE,
        Key::Char('q'),
        Command::RippleTrimTo(EditDirection::Previous),
    ),
    (
        Modifiers::NONE,
        Key::Char('w'),
        Command::RippleTrimTo(EditDirection::Next),
    ),
    (
        Modifiers::ALT,
        Key::Char('q'),
        Command::TrimTo(EditDirection::Previous),
    ),
    (
        Modifiers::ALT,
        Key::Char('w'),
        Command::TrimTo(EditDirection::Next),
    ),
    (Modifiers::NONE, Key::Char('e'), Command::ExtendEdit),
    (Modifiers::NONE, Key::Char('m'), Command::AddMarker),
    (Modifiers::ALT, Key::Char('m'), Command::AddClipMarker),
    (Modifiers::SHIFT, Key::Char('m'), Command::NextMarker),
    (CONTROL_SHIFT, Key::Char('m'), Command::PreviousMarker),
    (CONTROL_ALT, Key::Char('m'), Command::ClearMarkers),
    (Modifiers::NONE, Key::Char('u'), Command::KeyLanes),
    (Modifiers::SHIFT, Key::F(3), Command::GraphEditor),
    (
        Modifiers::NONE,
        Key::F(9),
        Command::Interpolate(cut_project::Interpolation::EasyEase),
    ),
    (
        Modifiers::SHIFT,
        Key::F(9),
        Command::Interpolate(cut_project::Interpolation::EasyEaseIn),
    ),
    (
        CONTROL_SHIFT,
        Key::F(9),
        Command::Interpolate(cut_project::Interpolation::EasyEaseOut),
    ),
];

static KEYMAP: LazyLock<ActionMap<Command>> = LazyLock::new(|| {
    BINDINGS
        .into_iter()
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
            Standard::Cut => Command::Cut,
            Standard::Copy => Command::Copy,
            Standard::Paste => Command::Paste,
            Standard::SelectAll => Command::SelectAll,
            Standard::SelectNone => Command::Deselect,
            _ => return None,
        })
    }

    pub(crate) fn chord(self) -> Option<Chord> {
        KEYMAP.chord_for(&self)
    }

    pub(crate) fn tooltip(self) -> Tooltip {
        Tooltip::new(wording::command(self)).shortcut(self.chord())
    }

    pub(crate) fn tool(self, symbol: MaterialSymbol, cut: &Cut) -> ToolButton {
        tool_button(symbol, wording::command(self))
            .shortcut(self.chord())
            .enabled(self.enabled(cut))
            .on_press(move |cut: &mut Cut| self.run(cut))
    }

    pub(crate) fn run(self, cut: &mut Cut) {
        match self {
            Command::New => cut.ask_to_start_over(),
            Command::Open => cut.ask_to_open(),
            Command::Import => cut.select_footage(),
            Command::Save => cut.save(),
            Command::SaveAs => cut.save_as(),
            Command::Export => cut.open_export(None),
            Command::ExportFrame => cut.open_export(Some(cut_export::Target::Still)),
            Command::Quit => cut.ask_to_close(),
            Command::Undo => cut.undo(),
            Command::Redo => cut.redo(),
            Command::Cut if cut.has_picked_keys() => cut.cut_keys(),
            Command::Copy if cut.has_picked_keys() => cut.copy_keys(),
            Command::Paste if cut.can_paste_keys() => cut.paste_keys(),
            Command::Deselect if cut.has_picked_keys() => cut.pick_keys(Vec::new()),
            Command::Delete if cut.has_picked_keys() => cut.delete_keys(),
            Command::Cut => cut.cut_selected(),
            Command::Copy => cut.copy(),
            Command::Paste => cut.paste(),
            Command::PasteAttributes => cut.open_pasting(),
            Command::PasteEffects => cut.paste_effects(),
            Command::RemoveEffects => cut.remove_effects(),
            Command::RelinkMedia => cut.open_relink(),
            Command::ReplaceMedia => cut.replace_media(),
            Command::SelectAll => cut.select_all(),
            Command::Deselect => cut.escape(),
            Command::Delete => cut.delete(false),
            Command::RippleDelete => cut.delete(true),
            Command::Split => cut.split_at_playhead(),
            Command::Snapping => cut.toggle_snapping(),
            Command::PlayPause => cut.play_pause(),
            Command::Stop => cut.stop(),
            Command::Pause => cut.pause(),
            Command::Shuttle(direction) => cut.shuttle(direction),
            Command::Step(frames) => cut.step(frames),
            Command::PreviousEdit => cut.go_to_edit(false),
            Command::NextEdit => cut.go_to_edit(true),
            Command::GoToStart => cut.go_to(0),
            Command::GoToEnd => cut.go_to(cut.project().end()),
            Command::MarkIn => cut.mark(Point::In),
            Command::MarkOut => cut.mark(Point::Out),
            Command::ClearMarks => cut.clear_marks(),
            Command::GoToIn => cut.go_to_mark(Point::In),
            Command::GoToOut => cut.go_to_mark(Point::Out),
            Command::ZoomIn => cut.zoom(true),
            Command::ZoomOut => cut.zoom(false),
            Command::ZoomToFit => cut.zoom_to_fit(),
            Command::Arm(Tool::Select) => cut.select_everywhere(),
            Command::Arm(tool) => cut.set_tool(tool),
            Command::Trimmer(trimmer) => cut.set_trimmer(trimmer),
            Command::Add(tool) => cut.add(tool),
            Command::AddRange => cut.add_range(Some(cut.range_shape())),
            Command::AddWhole => cut.add_range(None),
            Command::AddCamera => cut.add_camera(),
            Command::AddEffect(kind) => cut.add_effect(kind),
            Command::PreviousKey => cut.go_to_key(false),
            Command::NextKey => cut.go_to_key(true),
            Command::KeyLanes => cut.toggle_key_lanes(),
            Command::GraphEditor => cut.toggle_graph(),
            Command::Interpolate(interpolation) => cut.interpolate_keys(interpolation),
            Command::Quality(quality) => cut.set_quality(quality),
            Command::Thumbnails(mode) => cut.show_thumbnails(mode),
            Command::Waveform(scale) => cut.show_waveforms(scale),
            Command::LayerHeight(height) => cut.resize_layers(height),
            Command::RippleTrimTo(direction) => cut.ripple_trim_to_playhead(direction),
            Command::TrimTo(direction) => cut.trim_to_playhead(direction),
            Command::ExtendEdit => cut.extend_to_playhead(),
            Command::AddMarker => cut.add_marker(),
            Command::AddClipMarker => cut.add_clip_marker(),
            Command::PreviousMarker => cut.go_to_marker(false),
            Command::NextMarker => cut.go_to_marker(true),
            Command::EditMarker => cut.edit_marker_here(),
            Command::DeleteMarker => cut.delete_marker_here(),
            Command::ClearMarkers => cut.clear_markers(),
            Command::ProjectSettings => cut.open_reformat(),
        }
    }

    pub(crate) fn enabled(self, cut: &Cut) -> bool {
        if cut.is_modal() {
            return false;
        }
        let project = cut.project();
        let marks = project.marks();
        let selected = !cut.selected().is_empty();
        match self {
            Command::Undo => cut.document().undoable().is_some(),
            Command::Export | Command::ExportFrame => cut.can_export(),
            Command::Redo => cut.document().redoable().is_some(),
            Command::Delete => selected || cut.has_removable_transition(),
            Command::Cut | Command::Copy | Command::RippleDelete => selected,
            Command::Deselect => {
                selected
                    || cut.selected_edit_point().is_some()
                    || cut.tool() != Tool::Select
                    || cut.trimmer() != Trimmer::Select
                    || cut.marking().is_some()
            }
            Command::Paste => cut.can_paste() || cut.can_paste_keys(),
            Command::PasteAttributes => cut.can_paste_attributes(),
            Command::PasteEffects => cut.can_paste_effects(),
            Command::RemoveEffects => cut.can_remove_effects(),
            Command::RelinkMedia => cut.can_relink(),
            Command::ReplaceMedia => cut.can_replace_media(),
            Command::SelectAll | Command::Split | Command::ZoomToFit => !project.is_empty(),
            Command::PlayPause | Command::Shuttle(_) => !project.is_empty(),
            Command::ClearMarks => marks.mark_in.is_some() || marks.mark_out.is_some(),
            Command::GoToIn => marks.mark_in.is_some(),
            Command::GoToOut => marks.mark_out.is_some(),
            Command::AddEffect(_) => cut.selected_visual().is_some(),
            Command::PreviousKey | Command::NextKey => cut
                .selected_animated()
                .is_some_and(cut_project::Clip::is_animated),
            Command::KeyLanes | Command::GraphEditor => cut.selected_animated().is_some(),
            Command::Interpolate(_) => cut.has_picked_keys(),
            Command::RippleTrimTo(_) | Command::TrimTo(_) => !project.is_empty(),
            Command::ExtendEdit => selected || cut.selected_edit_point().is_some(),
            Command::AddClipMarker => cut.can_add_clip_marker(),
            Command::PreviousMarker | Command::NextMarker => !cut.marker_frames().is_empty(),
            Command::EditMarker | Command::DeleteMarker => cut.marker_here().is_some(),
            Command::ClearMarkers => !project.markers().is_empty(),
            _ => true,
        }
    }

    pub(crate) fn entry(self, cut: &Cut) -> MenuItem {
        self.entry_named(wording::command(self), cut)
    }

    pub(crate) fn entry_named(self, label: Label, cut: &Cut) -> MenuItem {
        let item = menu_item(label)
            .enabled(self.enabled(cut))
            .on_press(move |cut: &mut Cut| self.run(cut));
        match self.chord() {
            Some(chord) => item.shortcut(chord.to_string()),
            None => item,
        }
    }
}
