use launcher_install::{Installation, JavaChoice, Verify};
use launcher_instance::Window;
use rikka_app::prelude::*;
use rikka_dialog::{Chooser, ChooserResponse};
use rikka_net::CancelToken;
use rikka_ui::Element;
use rikka_widgets::{column, row};

use crate::editors::{memory_editor, window_editor};
use crate::launcher::Launcher;
use crate::preferences::Preferences;
use crate::target;
use crate::widgets::{heading, labelled};

pub(crate) struct SettingsPage {
    jvm: TextDocument,
    repairing: bool,
}

impl SettingsPage {
    pub(crate) fn new(preferences: &Preferences) -> SettingsPage {
        SettingsPage {
            jvm: TextDocument::new(&preferences.jvm_arguments),
            repairing: false,
        }
    }
}

impl Launcher {
    fn set_default_memory(&mut self, megabytes: f32) {
        self.preferences.memory_mb = Some(megabytes.round() as u32);
        self.save_preferences();
    }

    fn commit_jvm(&mut self) {
        let arguments = self.settings.jvm.text().trim().to_owned();
        if let Err(error) = launcher_game::split_arguments(&arguments) {
            self.fail(error.to_string());
            return;
        }
        self.preferences.jvm_arguments = arguments;
        self.save_preferences();
    }

    fn set_custom_window(&mut self, on: bool) {
        self.preferences.window = on.then_some(Window::DEFAULT);
        self.save_preferences();
    }

    fn set_default_window(&mut self, window: Window) {
        self.preferences.window = Some(window);
        self.save_preferences();
    }

    fn choose_java(&mut self) {
        let chooser = Chooser::open(tr!("launcher.settings.java-title").to_string())
            .parent(window_identifier());
        spawn_blocking(
            move || chooser.run().map_err(|error| error.to_string()),
            |launcher: &mut Launcher, picked: Result<ChooserResponse, String>| match picked {
                Ok(ChooserResponse::Files(files)) => {
                    if let Some(file) = files.into_iter().next() {
                        launcher.preferences.java = Some(file);
                        launcher.save_preferences();
                    }
                }
                Ok(ChooserResponse::Cancelled) => {}
                Err(error) => launcher.fail(error),
            },
        );
    }

    fn use_managed_java(&mut self) {
        self.preferences.java = None;
        self.save_preferences();
    }

    fn repair(&mut self) {
        let Some(services) = self.services() else {
            return;
        };
        self.settings.repairing = true;
        let java = self.preferences.java_choice();
        spawn_keyed(
            "launcher-repair",
            async move {
                services
                    .installer
                    .install(
                        &target::rikka(),
                        &java,
                        Verify::Contents,
                        CancelToken::new(),
                        |_| {},
                    )
                    .await
            },
            |launcher: &mut Launcher, repaired: launcher_install::Result<Installation>| {
                launcher.settings.repairing = false;
                match repaired {
                    Ok(_) => launcher.tell(tr!("launcher.settings.repaired")),
                    Err(error) => launcher.fail(error.to_string()),
                }
            },
        );
    }

    fn open_data_folder(&mut self) {
        let Some(services) = self.services() else {
            return;
        };
        if let Err(error) = rikka_open::open(&services.data.display().to_string()) {
            self.fail(error.to_string());
        }
    }
}

pub(crate) fn view(launcher: &Launcher) -> Element {
    let preferences = &launcher.preferences;
    let memory = preferences.memory();
    let target = target::rikka();
    let java = match preferences.java_choice() {
        JavaChoice::Managed => column! {
            text(tr!("launcher.settings.java-managed")).label();
            text(match cfg!(windows) {
                true => tr!("launcher.settings.java-managed-windows"),
                false => tr!("launcher.settings.java-managed-linux"),
            })
            .caption()
            .muted()
            .fill_width();
        }
        .gap(Space::HAIR)
        .grow(1.0)
        .any(),
        JavaChoice::Custom(path) => column! {
            text(tr!("launcher.settings.java-custom")).label();
            text(path.display().to_string()).caption().muted().fill_width();
        }
        .gap(Space::HAIR)
        .grow(1.0)
        .any(),
    };
    let window = preferences
        .window
        .map(|window| window_editor(window, Launcher::set_default_window));
    scrollable(
        column! {
            text(tr!("launcher.settings.title")).title().bold();
            heading(tr!("launcher.settings.game"));
            card(
                column! {
                    field(tr!("launcher.settings.version"), text(tr!(
                        "launcher.library.subtitle",
                        minecraft = target.minecraft.as_str(),
                        loader = target.fabric_loader.as_str()
                    )).label().muted());
                    field(
                        tr!("launcher.settings.repair"),
                        button(labelled(MaterialSymbol::BUILD, tr!("launcher.settings.repair-action")))
                            .secondary()
                            .loading(launcher.settings.repairing)
                            .on_press(Launcher::repair),
                    );
                    field(
                        tr!("launcher.settings.data"),
                        button(labelled(MaterialSymbol::FOLDER_OPEN, tr!("launcher.settings.open-data")))
                            .ghost()
                            .on_press(Launcher::open_data_folder),
                    );
                }
                .gap(Space::NORMAL)
                .fill_width(),
            );
            heading(tr!("launcher.settings.java"));
            card(
                column! {
                    row! {
                        java;
                        button(text(tr!("launcher.settings.java-choose")))
                            .secondary()
                            .on_press(Launcher::choose_java);
                        if preferences.java.is_some() {
                            button(text(tr!("launcher.settings.java-reset")))
                                .ghost()
                                .on_press(Launcher::use_managed_java);
                        }
                    }
                    .gap(Space::SNUG)
                    .align_items(Align::Center)
                    .fill_width();
                    text(tr!("launcher.settings.memory")).label();
                    memory_editor(memory, Launcher::set_default_memory);
                    text(tr!("launcher.settings.jvm")).label();
                    text_field(&launcher.settings.jvm)
                        .placeholder(tr!("launcher.options.jvm-placeholder"))
                        .on_change(|| {})
                        .on_submit(Launcher::commit_jvm)
                        .on_blur(Launcher::commit_jvm)
                        .fill_width();
                }
                .gap(Space::NORMAL)
                .fill_width(),
            );
            heading(tr!("launcher.settings.window"));
            card(
                column! {
                    field(
                        tr!("launcher.settings.custom-window"),
                        switch(preferences.window.is_some()).on_toggle(Launcher::set_custom_window),
                    );
                    window;
                }
                .gap(Space::NORMAL)
                .fill_width(),
            );
        }
        .gap(Space::NORMAL)
        .padding(Inset::all(Space::WIDE))
        .max_width(820.0)
        .fill_width(),
    )
    .fill()
    .any()
}
