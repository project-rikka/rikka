use rikka_compositor_scrolling::{Direction, Side};
use rikka_session::shortcuts::Shortcut;
use rikka_xdg::Terminals;

use super::KeyAction;
use crate::context::InputCtx;
use crate::context::{SessionRef, session_ctx, session_ref, windows_ctx, windows_ref};
use crate::session::DESKTOP_NAME;
use crate::windows::{ColumnCommand, Enlargement};

const TERMINAL_THREAD: &str = "rikka-terminal-spawn";

impl SessionRef<'_> {
    fn spawn_terminal(&self) {
        let environment = self.session_environment();
        let spawned = std::thread::Builder::new()
            .name(TERMINAL_THREAD.to_owned())
            .spawn(move || {
                let Some(terminal) = Terminals::session()
                    .map(|terminals| terminals.desktops([DESKTOP_NAME]))
                    .and_then(|terminals| terminals.preferred())
                else {
                    return tracing::warn!("no terminal emulator found");
                };
                let Some(mut command) = terminal.launch().command() else {
                    return tracing::warn!("{} has no command to run", terminal.entry().id);
                };
                for (name, value) in environment {
                    match value {
                        Some(value) => command.env(name, value),
                        None => command.env_remove(name),
                    };
                }
                if let Err(error) = rikka_open::spawn_detached(&mut command) {
                    tracing::warn!("cannot start {}: {error}", terminal.entry().id);
                }
            });
        if let Err(error) = spawned {
            tracing::warn!("cannot start {TERMINAL_THREAD}: {error}");
        }
    }
}

impl InputCtx<'_> {
    pub(crate) fn run_key_actions(&mut self) {
        for action in std::mem::take(&mut self.input.actions) {
            match action {
                KeyAction::Run(shortcut) => self.run_shortcut(shortcut),
                KeyAction::SwitchVt(vt) => self.backend.switch_vt(vt),
                KeyAction::Switched(target) => windows_ctx!(self).switched(target),
            }
        }
    }

    fn run_shortcut(&mut self, shortcut: Shortcut) {
        match shortcut {
            Shortcut::Quit => {
                tracing::info!("quit requested via keybinding");
                self.core.stop();
            }
            Shortcut::StopDesktop => session_ctx!(self).stop_desktop(),
            Shortcut::CloseWindow => {
                if let Some(window) = windows_ref!(self).focused_window() {
                    windows_ctx!(self).close_window(window);
                }
            }
            Shortcut::ToggleFullscreen => {
                if let Some(window) = windows_ref!(self).focused_window() {
                    windows_ctx!(self).toggle(window, Enlargement::Fullscreen);
                }
            }
            Shortcut::ToggleMaximize => {
                if let Some(window) = windows_ref!(self).focused_window() {
                    windows_ctx!(self).toggle(window, Enlargement::Maximized);
                }
            }
            Shortcut::ToggleFloating => {
                if let Some(window) = windows_ref!(self).focused_window() {
                    windows_ctx!(self).toggle_floating(window);
                }
            }
            Shortcut::SwitchWindows | Shortcut::SwitchWindowsBack => {}
            Shortcut::FocusColumnLeft => {
                windows_ctx!(self).command_columns(ColumnCommand::Focus(Direction::Left))
            }
            Shortcut::FocusColumnRight => {
                windows_ctx!(self).command_columns(ColumnCommand::Focus(Direction::Right));
            }
            Shortcut::FocusWindowUp => {
                windows_ctx!(self).command_columns(ColumnCommand::Focus(Direction::Up))
            }
            Shortcut::FocusWindowDown => {
                windows_ctx!(self).command_columns(ColumnCommand::Focus(Direction::Down))
            }
            Shortcut::MoveColumnLeft => {
                windows_ctx!(self).command_columns(ColumnCommand::Shift(Direction::Left))
            }
            Shortcut::MoveColumnRight => {
                windows_ctx!(self).command_columns(ColumnCommand::Shift(Direction::Right));
            }
            Shortcut::MoveWindowUp => {
                windows_ctx!(self).command_columns(ColumnCommand::Shift(Direction::Up))
            }
            Shortcut::MoveWindowDown => {
                windows_ctx!(self).command_columns(ColumnCommand::Shift(Direction::Down))
            }
            Shortcut::ConsumeOrExpelLeft => {
                windows_ctx!(self).command_columns(ColumnCommand::ConsumeOrExpel(Side::Left));
            }
            Shortcut::ConsumeOrExpelRight => {
                windows_ctx!(self).command_columns(ColumnCommand::ConsumeOrExpel(Side::Right));
            }
            Shortcut::SwitchColumnWidth => {
                windows_ctx!(self).command_columns(ColumnCommand::CycleWidth)
            }
            Shortcut::SpawnTerminal => session_ref!(self).spawn_terminal(),
            Shortcut::ShowWorkspace(number) => {
                windows_ctx!(self).show_workspace(usize::from(number))
            }
            Shortcut::MoveToWorkspace(number) => {
                windows_ctx!(self).send_to_workspace(usize::from(number))
            }
            Shortcut::ToggleOverview
            | Shortcut::EditDesk
            | Shortcut::ToggleLauncher
            | Shortcut::ToggleKeyboard
            | Shortcut::ClipboardHistory
            | Shortcut::EmojiPicker
            | Shortcut::ScreenshotScreen
            | Shortcut::ScreenshotRegion
            | Shortcut::RecordScreen
            | Shortcut::RecordRegion
            | Shortcut::SaveReplay
            | Shortcut::ChooseReplay
            | Shortcut::ToggleTranslate
            | Shortcut::TranslateRegion
            | Shortcut::ColorPicker
            | Shortcut::ScanCode
            | Shortcut::LockScreen => self.session.announce(shortcut),
        }
    }
}
