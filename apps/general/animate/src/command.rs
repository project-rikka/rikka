use animate_inspector::ClothEditing;
use std::sync::LazyLock;

use rikka_app::prelude::*;
use rikka_input::{ActionMap, Chord};
use rikka_ui::Command as Standard;

use crate::animate::{Animate, LookPart, Starter};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Command {
    Add,
    AddStage,
    AddAccessory,
    AddLiquid,
    DuplicateLiquid,
    Close,
    PlayPause,
    Repeat,
    StepBack,
    StepForward,
    LeapBack,
    LeapForward,
    PreviousKey,
    NextKey,
    GoToStart,
    GoToEnd,
    ZoomIn,
    ZoomOut,
    ZoomToFit,
    FrameEveryone,
    RestView,
    OutputSize,
    ExportVideo,
    Physics(bool),
    ResetPhysics,
    Liquids(bool),
    RecomputeLiquids,
    Save,
    SaveCloth,
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    DeleteKeys,
    Register,
    ResetPose,
    Inspector,
    Library,
    Assistant,
    SwitchMode,
    Pin,
    ReleaseHold,
    NodeDock,
    SplitDock,
    ResetLook(LookPart, Starter),
}

const SHIFT_CONTROL: Modifiers = Modifiers {
    shift: true,
    control: true,
    ..Modifiers::NONE
};

const BINDINGS: [(Modifiers, Key, Command); 34] = [
    (Modifiers::CONTROL, Key::Char('o'), Command::Add),
    (Modifiers::NONE, Key::Space, Command::PlayPause),
    (Modifiers::NONE, Key::ArrowLeft, Command::StepBack),
    (Modifiers::NONE, Key::ArrowRight, Command::StepForward),
    (Modifiers::SHIFT, Key::ArrowLeft, Command::LeapBack),
    (Modifiers::SHIFT, Key::ArrowRight, Command::LeapForward),
    (Modifiers::NONE, Key::ArrowUp, Command::PreviousKey),
    (Modifiers::NONE, Key::ArrowDown, Command::NextKey),
    (Modifiers::NONE, Key::Home, Command::GoToStart),
    (Modifiers::NONE, Key::End, Command::GoToEnd),
    (Modifiers::NONE, Key::Char('='), Command::ZoomIn),
    (Modifiers::SHIFT, Key::Char('='), Command::ZoomIn),
    (Modifiers::NONE, Key::Char('+'), Command::ZoomIn),
    (Modifiers::SHIFT, Key::Char('+'), Command::ZoomIn),
    (Modifiers::NONE, Key::Char('-'), Command::ZoomOut),
    (Modifiers::NONE, Key::Char('\\'), Command::ZoomToFit),
    (Modifiers::NONE, Key::Char('f'), Command::FrameEveryone),
    (Modifiers::CONTROL, Key::Char('s'), Command::Save),
    (SHIFT_CONTROL, Key::Char('e'), Command::ExportVideo),
    (Modifiers::CONTROL, Key::Char('z'), Command::Undo),
    (Modifiers::CONTROL, Key::Char('y'), Command::Redo),
    (SHIFT_CONTROL, Key::Char('z'), Command::Redo),
    (Modifiers::CONTROL, Key::Char('x'), Command::Cut),
    (Modifiers::CONTROL, Key::Char('c'), Command::Copy),
    (Modifiers::CONTROL, Key::Char('v'), Command::Paste),
    (Modifiers::NONE, Key::Delete, Command::DeleteKeys),
    (Modifiers::NONE, Key::Enter, Command::Register),
    (Modifiers::CONTROL, Key::Char('i'), Command::Inspector),
    (Modifiers::CONTROL, Key::Char('l'), Command::Library),
    (Modifiers::CONTROL, Key::Char('k'), Command::Assistant),
    (Modifiers::CONTROL, Key::Char('e'), Command::SwitchMode),
    (Modifiers::NONE, Key::Char('p'), Command::Pin),
    (Modifiers::NONE, Key::Char('g'), Command::ReleaseHold),
    (SHIFT_CONTROL, Key::Char('g'), Command::NodeDock),
];

static KEYMAP: LazyLock<ActionMap<Command>> = LazyLock::new(|| {
    BINDINGS
        .into_iter()
        .fold(ActionMap::new(), |map, (modifiers, key, command)| {
            map.bind(Chord::new(modifiers, key), command)
        })
});

impl Command {
    pub(crate) fn pressed(modifiers: Modifiers, key: Key) -> Option<Self> {
        KEYMAP.resolve(modifiers, key).copied()
    }

    pub(crate) fn from_standard(standard: Standard) -> Option<Command> {
        Some(match standard {
            Standard::Undo => Command::Undo,
            Standard::Redo => Command::Redo,
            Standard::Cut => Command::Cut,
            Standard::Copy => Command::Copy,
            Standard::Paste => Command::Paste,
            _ => return None,
        })
    }

    fn chord(self) -> Option<Chord> {
        KEYMAP.chord_for(&self)
    }

    pub(crate) fn tool(
        self,
        symbol: MaterialSymbol,
        label: Label,
        animate: &Animate,
    ) -> ToolButton {
        tool_button(symbol, label)
            .shortcut(self.chord())
            .enabled(self.enabled(animate))
            .on_press(move |animate: &mut Animate| self.run(animate))
    }

    pub(crate) fn run(self, animate: &mut Animate) {
        match self {
            Self::Add => animate.add(),
            Self::AddStage => animate.choose_stage(),
            Self::AddAccessory => animate.choose_accessories(),
            Self::AddLiquid => animate.add_liquid(),
            Self::DuplicateLiquid => animate.duplicate_liquid(),
            Self::Close => request_window(WindowRequest::Close),
            Self::PlayPause => animate.play_pause(),
            Self::Repeat => animate.toggle_repeat(),
            Self::StepBack => animate.step(-1),
            Self::StepForward => animate.step(1),
            Self::LeapBack => animate.step(-10),
            Self::LeapForward => animate.step(10),
            Self::PreviousKey => animate.go_to_key(false),
            Self::NextKey => animate.go_to_key(true),
            Self::GoToStart => animate.go_to(0),
            Self::GoToEnd => animate.go_to_end(),
            Self::ZoomIn => animate.zoom(true),
            Self::ZoomOut => animate.zoom(false),
            Self::ZoomToFit => animate.zoom_to_fit(),
            Self::FrameEveryone => animate.frame_all(),
            Self::RestView => animate.rest_view(),
            Self::OutputSize => animate.output_mut().start_sizing(),
            Self::ExportVideo => animate.open_export(),
            Self::Physics(on) => animate.set_physics(on),
            Self::ResetPhysics => animate.reset_physics(),
            Self::Liquids(on) => animate.set_pouring(on),
            Self::RecomputeLiquids => animate.recompute_liquids(),
            Self::Save => animate.save_motion(),
            Self::SaveCloth => animate.save_cloth(),
            Self::Undo => animate.undo(),
            Self::Redo => animate.redo(),
            Self::Cut => animate.cut_keys(),
            Self::Copy => animate.copy_keys(),
            Self::Paste => animate.paste_keys(),
            Self::DeleteKeys => animate.delete_keys(),
            Self::Register => animate.register(),
            Self::ResetPose => animate.reset_pose(),
            Self::Inspector => animate.toggle_inspector(),
            Self::Library => animate.toggle_library(),
            Self::Assistant => animate.open_assistant(),
            Self::SwitchMode => animate.switch_mode(),
            Self::Pin => animate.pin_chosen(),
            Self::ReleaseHold => animate.release_holds(),
            Self::NodeDock => animate.toggle_node_dock(),
            Self::SplitDock => animate.split_dock(),
            Self::ResetLook(part, starter) => animate.reset_look(part, starter),
        }
    }

    pub(crate) fn enabled(self, animate: &Animate) -> bool {
        match self {
            Self::FrameEveryone => {
                !animate.cast().figures().is_empty() || animate.cast().props().next().is_some()
            }
            Self::ResetPhysics => {
                animate.is_physics_enabled() && !animate.cast().figures().is_empty()
            }
            Self::PlayPause | Self::PreviousKey | Self::NextKey | Self::GoToEnd => {
                animate.transport().length() > 0.0
            }
            Self::Save => animate.can_save(),
            Self::SaveCloth => animate.can_save_cloth(),
            Self::ExportVideo => animate.can_export(),
            Self::Undo => animate.can_undo(),
            Self::Redo => animate.can_redo(),
            Self::Cut | Self::Copy => animate.has_picks(),
            Self::DeleteKeys => animate.has_picks() || animate.chosen_hold().is_some(),
            Self::Paste => animate.can_paste(),
            Self::Register => animate.can_register(),
            Self::ResetPose => animate.can_reset_pose(),
            Self::Pin => animate.can_pin(),
            Self::ReleaseHold => animate.can_release_hold(),
            Self::DuplicateLiquid => animate.can_duplicate_liquid(),
            Self::RecomputeLiquids => animate.can_recompute_liquids(),
            Self::Add
            | Self::AddStage
            | Self::AddAccessory
            | Self::AddLiquid
            | Self::Close
            | Self::Repeat
            | Self::ZoomIn
            | Self::ZoomOut
            | Self::ZoomToFit
            | Self::RestView
            | Self::OutputSize
            | Self::Physics(_)
            | Self::Liquids(_)
            | Self::StepBack
            | Self::StepForward
            | Self::LeapBack
            | Self::LeapForward
            | Self::GoToStart
            | Self::Inspector
            | Self::Library
            | Self::Assistant
            | Self::SwitchMode
            | Self::NodeDock
            | Self::SplitDock
            | Self::ResetLook(..) => true,
        }
    }

    pub(crate) fn entry(self, label: Label, animate: &Animate) -> MenuItem {
        let item = menu_item(label)
            .enabled(self.enabled(animate))
            .on_press(move |animate: &mut Animate| self.run(animate));
        match self.chord() {
            Some(chord) => item.shortcut(chord.to_string()),
            None => item,
        }
    }
}
