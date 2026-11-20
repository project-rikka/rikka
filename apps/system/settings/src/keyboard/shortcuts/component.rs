use rikka_app::prelude::*;
use rikka_input::Chord;
use rikka_session::shortcuts::{DOMAIN, Group};
use rikka_settings::seam::Announcement;
use rikka_widgets::{column, row};

use crate::domain::{Domain, DomainPage};
use crate::keyboard::shortcuts::binding::{Binding, Bindings};
use crate::notice::domain_notice;
use crate::session::Session;

const FIELD: f32 = 200.0;

#[derive(Debug)]
struct Conflict {
    name: String,
    chord: Chord,
    held_by: String,
}

pub(crate) struct Shortcuts {
    domain: Domain,
    bindings: Bindings,
    recording: Option<String>,
    conflict: Option<Conflict>,
}

impl DomainPage for Shortcuts {
    fn domain(&self) -> &Domain {
        &self.domain
    }

    fn domain_mut(&mut self) -> &mut Domain {
        &mut self.domain
    }

    fn announced(&mut self, announcement: Announcement) {
        let bindings = self.bindings.clone();
        self.domain.follow(announcement);
        self.reread();
        if self.bindings == bindings {
            skip_view();
        }
    }
}

impl Shortcuts {
    pub(crate) fn new() -> Self {
        Self {
            domain: Domain::new(DOMAIN),
            bindings: Bindings::default(),
            recording: None,
            conflict: None,
        }
    }

    pub(crate) fn watch(&mut self, session: &Session) {
        self.domain.open::<Self>(session);
        self.recording = None;
        self.conflict = None;
        self.reread();
    }

    fn reread(&mut self) {
        self.bindings = Bindings::read(&self.domain);
    }

    fn bind(&mut self, name: &str, chord: Option<Chord>) {
        if self.bindings.named(name).map(Binding::chord) == Some(chord) {
            return;
        }
        let value = chord.map(|chord| chord.to_string()).unwrap_or_default();
        self.domain.set(name, value);
        self.reread();
    }

    fn capture(&mut self, name: String, chord: Chord) {
        self.recording = None;
        match self.bindings.also_bound(chord, &name) {
            Some(held_by) => {
                self.conflict = Some(Conflict {
                    name,
                    chord,
                    held_by: held_by.name().to_owned(),
                });
            }
            None => self.bind(&name, Some(chord)),
        }
    }

    fn replace(&mut self) {
        let Some(conflict) = self.conflict.take() else {
            return;
        };
        self.bind(&conflict.held_by, None);
        self.bind(&conflict.name, Some(conflict.chord));
    }

    fn record(&mut self, name: String) {
        self.conflict = None;
        self.recording = Some(name);
    }

    fn clear(&mut self, name: &str) {
        self.conflict = None;
        self.bind(name, None);
    }

    fn reset(&mut self, name: &str) {
        self.conflict = None;
        self.domain.reset(name);
        self.reread();
    }
}

pub(crate) fn view(shortcuts: &Shortcuts) -> impl View {
    column! {
        domain_notice(
            &shortcuts.domain,
            "settings.shortcuts.unavailable",
            "settings.shortcuts.refused",
        );
        if !shortcuts.bindings.is_empty() {
            for group in shortcuts.bindings.groups().iter().copied() {
                group_card(shortcuts, group);
            }
        }
    }
    .gap(Space::ROOMY)
    .fill_width()
}

fn group_card(shortcuts: &Shortcuts, group: Option<Group>) -> impl View {
    section(
        group_title(group),
        column! {
            for binding in shortcuts.bindings.in_group(group) {
                row(shortcuts, binding);
                if let Some(conflict) = shortcuts
                    .conflict
                    .as_ref()
                    .filter(|at| at.name == binding.name())
                {
                    warning(shortcuts, conflict);
                }
            }
        }
        .gap(Space::SNUG),
    )
}

fn row(shortcuts: &Shortcuts, binding: &Binding) -> impl View {
    let name = binding.name().to_owned();
    let recording = shortcuts.recording.as_deref() == Some(binding.name());
    let recorded = name.clone();
    let captured = name.clone();
    let cleared = name.clone();
    let controls = row! {
        shortcut_field(chord_text(binding.chord()))
            .recording(recording)
            .prompt(tr!("settings.shortcuts.recording"))
            .warned(binding.shared())
            .on_record(move |shortcuts: &mut Shortcuts| shortcuts.record(recorded.clone()))
            .on_capture(move |shortcuts: &mut Shortcuts, modifiers: Modifiers, key: Key| {
                shortcuts.capture(captured.clone(), Chord::new(modifiers, key));
            })
            .on_cancel(|shortcuts: &mut Shortcuts| shortcuts.recording = None)
            .width(FIELD);
        button(icon(MaterialSymbol::BACKSPACE))
            .ghost()
            .icon_only()
            .enabled(binding.chord().is_some())
            .on_press(move |shortcuts: &mut Shortcuts| shortcuts.clear(&cleared));
        button(icon(MaterialSymbol::RESTART_ALT))
            .ghost()
            .icon_only()
            .enabled(!binding.is_default())
            .on_press(move |shortcuts: &mut Shortcuts| shortcuts.reset(&name));
    }
    .gap(Space::SNUG)
    .align_items(Align::Center);
    field(label(binding), controls)
}

fn warning(shortcuts: &Shortcuts, conflict: &Conflict) -> impl View {
    let held_by = shortcuts
        .bindings
        .named(&conflict.held_by)
        .map(label)
        .unwrap_or_else(|| Label::from(conflict.held_by.as_str()));
    let chord = conflict.chord.to_string();
    let message = tr!(
        "settings.shortcuts.conflict",
        chord = &chord,
        command = held_by.as_str()
    );
    container(
        column! {
            text(message).label();
            row! {
                spacer();
                button(text(tr!("settings.shortcuts.keep")))
                    .secondary()
                    .small()
                    .on_press(|shortcuts: &mut Shortcuts| shortcuts.conflict = None);
                button(text(tr!("settings.shortcuts.replace")))
                    .small()
                    .on_press(|shortcuts: &mut Shortcuts| shortcuts.replace());
            }
            .gap(Space::SNUG)
            .align_items(Align::Center);
        }
        .gap(Space::SNUG)
        .fill_width(),
    )
    .radius(Corner::Panel)
    .padding(Space::NORMAL)
    .border(1.0, palette().warning)
    .fill_width()
}

impl std::fmt::Debug for Shortcuts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Shortcuts")
            .field("recording", &self.recording)
            .field("conflict", &self.conflict)
            .finish_non_exhaustive()
    }
}

fn label(binding: &Binding) -> Label {
    let key = format!("settings.shortcuts.command.{}", binding.name());
    match rikka_i18n::translate(&key) {
        Some(named) => Label::from(named),
        None => Label::from(binding.summary()),
    }
}

fn chord_text(chord: Option<Chord>) -> Label {
    match chord {
        Some(chord) => Label::from(chord.to_string()),
        None => tr!("settings.shortcuts.unbound"),
    }
}

fn group_title(group: Option<Group>) -> Label {
    match group {
        Some(Group::Session) => tr!("settings.shortcuts.group.session"),
        Some(Group::Window) => tr!("settings.shortcuts.group.window"),
        Some(Group::Columns) => tr!("settings.shortcuts.group.columns"),
        Some(Group::Workspace) => tr!("settings.shortcuts.group.workspace"),
        Some(Group::Desktop) => tr!("settings.shortcuts.group.desktop"),
        None => tr!("settings.shortcuts.group.other"),
    }
}
