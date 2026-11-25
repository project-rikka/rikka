use launcher_instance::{Instance, Window};
use rikka_app::prelude::*;
use rikka_dialog::{Chooser, ChooserResponse, Filter};
use rikka_ui::Element;
use rikka_widgets::{column, row};

use crate::editors::{memory_editor, window_editor};
use crate::launcher::Launcher;
use crate::services::update;
use crate::widgets::{heading, labelled};

pub(crate) struct OptionsForm {
    name: TextDocument,
    memory: Option<u32>,
    jvm: Option<TextDocument>,
    window: Option<Window>,
}

impl Launcher {
    pub(crate) fn open_options(&mut self) {
        let Some(instance) = self.selected.as_deref().and_then(|id| self.instance(id)) else {
            return;
        };
        self.instance.options = Some(OptionsForm {
            name: TextDocument::new(&instance.config.name),
            memory: instance.config.memory_mb,
            jvm: instance
                .config
                .jvm_arguments
                .as_deref()
                .map(TextDocument::new),
            window: instance.config.window,
        });
    }

    fn form(&mut self) -> Option<&mut OptionsForm> {
        self.instance.options.as_mut()
    }

    fn override_memory(&mut self, on: bool) {
        let default = self.preferences.memory();
        if let Some(form) = self.form() {
            form.memory = on.then_some(default);
        }
    }

    fn set_memory(&mut self, megabytes: f32) {
        if let Some(form) = self.form() {
            form.memory = Some(megabytes.round() as u32);
        }
    }

    fn override_jvm(&mut self, on: bool) {
        let default = self.preferences.jvm_arguments.clone();
        if let Some(form) = self.form() {
            form.jvm = on.then(|| TextDocument::new(&default));
        }
    }

    fn override_window(&mut self, on: bool) {
        let default = self.preferences.window.unwrap_or(Window::DEFAULT);
        if let Some(form) = self.form() {
            form.window = on.then_some(default);
        }
    }

    fn set_window(&mut self, window: Window) {
        if let Some(form) = self.form() {
            form.window = Some(window);
        }
    }

    fn save_options(&mut self) {
        let (Some(id), Some(services), Some(form)) = (
            self.selected_id(),
            self.services(),
            self.instance.options.as_ref(),
        ) else {
            return;
        };
        let name = form.name.text().trim().to_owned();
        if name.is_empty() {
            self.fail(tr!("launcher.options.name-required"));
            return;
        }
        let jvm = form.jvm.as_ref().map(|jvm| jvm.text().trim().to_owned());
        if let Some(arguments) = &jvm
            && let Err(error) = launcher_game::split_arguments(arguments)
        {
            self.fail(error.to_string());
            return;
        }
        let memory = form.memory;
        let window = form.window;
        spawn(
            update(services, id, move |config| {
                config.name = name;
                config.memory_mb = memory;
                config.jvm_arguments = jvm;
                config.window = window;
            }),
            |launcher: &mut Launcher, saved: launcher_instance::Result<Instance>| match saved {
                Ok(instance) => {
                    launcher.replace_instance(instance);
                    launcher.tell(tr!("launcher.options.saved"));
                }
                Err(error) => launcher.fail(error.to_string()),
            },
        );
    }

    fn change_icon(&mut self) {
        let (Some(id), Some(services)) = (self.selected_id(), self.services()) else {
            return;
        };
        let chooser = Chooser::open(tr!("launcher.options.icon-title").to_string())
            .parent(window_identifier())
            .filter(
                Filter::named(tr!("launcher.options.icon-filter").to_string())
                    .mime_type("image/png")
                    .mime_type("image/jpeg")
                    .pattern("*.png")
                    .pattern("*.jpg"),
            );
        spawn_blocking(
            move || -> Result<Option<Instance>, String> {
                let ChooserResponse::Files(files) =
                    chooser.run().map_err(|error| error.to_string())?
                else {
                    return Ok(None);
                };
                let Some(file) = files.first() else {
                    return Ok(None);
                };
                let mut instance = services
                    .instances
                    .get(&id)
                    .map_err(|error| error.to_string())?;
                instance.set_icon(file).map_err(|error| error.to_string())?;
                Ok(Some(instance))
            },
            |launcher: &mut Launcher, changed: Result<Option<Instance>, String>| match changed {
                Ok(Some(instance)) => launcher.replace_instance(instance),
                Ok(None) => {}
                Err(error) => launcher.fail(error),
            },
        );
    }
}

fn toggled_field(
    label: Label,
    on: bool,
    toggle: fn(&mut Launcher, bool),
    control: Option<Element>,
) -> Element {
    column! {
        field(label, switch(on).on_toggle(toggle));
        control;
    }
    .gap(Space::SNUG)
    .fill_width()
    .any()
}

pub(super) fn view(launcher: &Launcher, id: &str) -> Element {
    let Some(form) = &launcher.instance.options else {
        return Element::empty();
    };
    let deleted = id.to_owned();
    let memory = form
        .memory
        .map(|megabytes| memory_editor(megabytes, Launcher::set_memory));
    let jvm = form.jvm.as_ref().map(|jvm| {
        text_field(jvm)
            .placeholder(tr!("launcher.options.jvm-placeholder"))
            .on_change(|| {})
            .fill_width()
            .any()
    });
    let window = form
        .window
        .map(|window| window_editor(window, Launcher::set_window));
    scrollable(
        column! {
            heading(tr!("launcher.options.general"));
            card(
                column! {
                    field(
                        tr!("launcher.options.name"),
                        text_field(&form.name).on_change(|| {}).width(Length::px(280.0)),
                    );
                    field(
                        tr!("launcher.options.icon"),
                        button(labelled(MaterialSymbol::IMAGE, tr!("launcher.options.change-icon")))
                            .secondary()
                            .on_press(Launcher::change_icon),
                    );
                }
                .gap(Space::NORMAL)
                .fill_width(),
            );
            heading(tr!("launcher.options.java"));
            card(
                column! {
                    toggled_field(tr!("launcher.options.custom-memory"), form.memory.is_some(), Launcher::override_memory, memory);
                    toggled_field(tr!("launcher.options.custom-jvm"), form.jvm.is_some(), Launcher::override_jvm, jvm);
                    toggled_field(tr!("launcher.options.custom-window"), form.window.is_some(), Launcher::override_window, window);
                }
                .gap(Space::ROOMY)
                .fill_width(),
            );
            row! {
                button(labelled(MaterialSymbol::DELETE, tr!("launcher.options.delete")))
                    .danger()
                    .on_press(move |launcher: &mut Launcher| launcher.ask_delete_instance(deleted.clone()));
                spacer().grow(1.0);
                button(text(tr!("launcher.action.save")))
                    .primary()
                    .on_press(Launcher::save_options);
            }
            .align_items(Align::Center)
            .fill_width();
        }
        .gap(Space::NORMAL)
        .padding(Inset::edges(Space::NONE, Space::WIDE, Space::WIDE, Space::WIDE))
        .fill_width(),
    )
    .fill()
    .any()
}
