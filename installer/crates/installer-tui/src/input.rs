use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use installer_core::Step;

use crate::{
    app::{App, Outcome},
    form::Field,
    overlay::{Confirm, Decision, Overlay},
};

impl App {
    pub fn key(&mut self, key: KeyEvent) {
        if key.kind == KeyEventKind::Release {
            return;
        }

        if quits(&key) {
            self.ask_to_quit();
            return;
        }

        if self.is_reading() {
            return;
        }

        if key.code == KeyCode::F(1) && self.overlay().is_none() {
            self.show(Overlay::Help);
            return;
        }

        if self.overlay().is_some() {
            self.key_overlay(key);
        } else if self.step() == Step::Install {
            self.key_install(key);
        } else {
            self.key_form(key);
        }
    }

    fn ask_to_quit(&mut self) {
        let running = self.step() == Step::Install && !self.install().finished();
        let body = if running {
            vec![
                "The install is still running.".to_owned(),
                "Leaving now leaves the disk half written.".to_owned(),
            ]
        } else {
            vec!["The answers are not kept.".to_owned()]
        };
        self.show(Overlay::Confirm(Confirm::new(
            "Quit the installer",
            body,
            Decision::Quit,
        )));
    }

    fn key_form(&mut self, key: KeyEvent) {
        let typing = matches!(self.form().focused(), Some(Field::Text { .. }));
        let list = self.form().focused().is_some_and(Field::is_list);

        match key.code {
            KeyCode::Enter => self.forward(),
            KeyCode::Esc => self.retreat(),
            KeyCode::Tab => self.form_mut().step_focus(1),
            KeyCode::BackTab => self.form_mut().step_focus(-1),

            KeyCode::Down if list => self.cycle(1),
            KeyCode::Up if list => self.cycle(-1),
            KeyCode::Down => self.form_mut().step_focus(1),
            KeyCode::Up => self.form_mut().step_focus(-1),

            KeyCode::Left => self.cycle(-1),
            KeyCode::Right => self.cycle(1),
            KeyCode::Home => {
                if let Some(field) = self.form_mut().focused_mut() {
                    field.go_home();
                }
            }
            KeyCode::End => {
                if let Some(field) = self.form_mut().focused_mut() {
                    field.go_end();
                }
            }
            KeyCode::Backspace => {
                if let Some(field) = self.form_mut().focused_mut() {
                    field.backspace();
                }
                self.commit();
            }

            KeyCode::Char('?') if !typing => self.show(Overlay::Help),

            KeyCode::Char(' ') if !typing => self.activate(),
            KeyCode::Char(ch) if typing && !key.modifiers.contains(KeyModifiers::CONTROL) => {
                if let Some(field) = self.form_mut().focused_mut() {
                    field.insert(ch);
                }
                self.commit();
            }
            _ => {}
        }
    }

    fn forward(&mut self) {
        if self.form().on_last() {
            self.advance();
        } else {
            self.form_mut().step_focus(1);
        }
    }

    fn activate(&mut self) {
        if matches!(self.form().focused(), Some(Field::Picker { .. })) {
            self.open_picker();
            return;
        }
        if let Some(field) = self.form_mut().focused_mut() {
            field.activate();
        }
        self.refresh();
    }

    fn cycle(&mut self, delta: isize) {
        let text = matches!(self.form().focused(), Some(Field::Text { .. }));
        if let Some(field) = self.form_mut().focused_mut() {
            field.cycle(delta);
        }
        if !text {
            self.refresh();
        }
    }

    fn key_overlay(&mut self, key: KeyEvent) {
        match self.overlay() {
            Some(Overlay::Help) => self.dismiss(),
            Some(Overlay::Picker(_)) => self.key_picker(key),
            Some(Overlay::Confirm(_)) => self.key_confirm(key),
            None => {}
        }
    }

    fn key_picker(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Enter => return self.take_pick(),
            KeyCode::Esc => return self.dismiss(),
            _ => {}
        }

        let Some(picker) = self.picker_mut() else {
            return;
        };
        match key.code {
            KeyCode::Up => picker.move_by(-1),
            KeyCode::Down => picker.move_by(1),
            KeyCode::PageUp => picker.move_by(-10),
            KeyCode::PageDown => picker.move_by(10),
            KeyCode::Backspace => picker.pop(),
            KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => picker.push(ch),
            _ => {}
        }
    }

    fn key_confirm(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('n' | 'N') => return self.dismiss(),
            KeyCode::Enter => return self.decide(),
            _ => {}
        }

        let Some(confirm) = self.confirm_mut() else {
            return;
        };
        match key.code {
            KeyCode::Left | KeyCode::Right | KeyCode::Tab | KeyCode::BackTab => {
                confirm.yes = !confirm.yes;
            }
            KeyCode::Char('y' | 'Y') => {
                confirm.yes = true;
                self.decide();
            }
            _ => {}
        }
    }

    fn take_pick(&mut self) {
        let Some(picker) = self.picker_mut() else {
            return;
        };
        let (field, chosen) = (picker.field, picker.selected());
        self.dismiss();

        let Some(chosen) = chosen else {
            return;
        };
        if let Some(Field::Picker { index, .. }) = self.form_mut().get_mut(field) {
            *index = chosen;
        }
        self.refresh();
    }

    fn decide(&mut self) {
        let Some(confirm) = self.confirm_mut() else {
            return;
        };
        let (yes, action) = (confirm.yes, confirm.action);
        self.dismiss();
        if !yes {
            return;
        }
        match action {
            Decision::BeginInstall => self.begin_install(),
            Decision::Quit => self.quit(Outcome::Quit),
        }
    }

    fn key_install(&mut self, key: KeyEvent) {
        let page = self.install().log().viewport() as isize;
        match key.code {
            KeyCode::Up => self.install_mut().log_mut().scroll(-1),
            KeyCode::Down => self.install_mut().log_mut().scroll(1),
            KeyCode::PageUp => self.install_mut().log_mut().scroll(-page),
            KeyCode::PageDown => self.install_mut().log_mut().scroll(page),
            KeyCode::End => self.install_mut().log_mut().scroll(isize::MAX),

            KeyCode::Char('r' | 'R') if matches!(self.install().outcome(), Some(Ok(()))) => {
                self.quit(Outcome::Reboot);
            }
            KeyCode::Enter | KeyCode::Char('q' | 'Q') if self.install().finished() => {
                self.quit(Outcome::Quit);
            }
            _ => {}
        }
    }
}

fn quits(key: &KeyEvent) -> bool {
    key.modifiers.contains(KeyModifiers::CONTROL)
        && matches!(key.code, KeyCode::Char('q') | KeyCode::Char('c'))
}
