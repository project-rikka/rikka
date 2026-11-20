use rikka_types::{Key, Modifiers};
use rikka_ui::{Command, Element, Handler, Styled, View};
use rikka_widgets::container;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Trigger {
    Command(Command),
    Chord(Key, Modifiers),
}

#[derive(Clone, Default)]
pub(crate) struct Bindings {
    bound: Vec<(Trigger, Handler)>,
}

impl Bindings {
    pub(crate) fn command(&mut self, command: Command, handler: Handler) {
        self.bind(Trigger::Command(command), handler);
    }

    pub(crate) fn chord(&mut self, key: Key, modifiers: Modifiers, handler: Handler) {
        self.bind(Trigger::Chord(key.to_lowercase(), modifiers), handler);
    }

    fn bind(&mut self, trigger: Trigger, handler: Handler) {
        self.bound.retain(|(bound, _)| *bound != trigger);
        self.bound.push((trigger, handler));
    }

    pub(crate) fn around(&self, window: Element) -> Element {
        if self.bound.is_empty() {
            return window;
        }
        self.bound
            .iter()
            .fold(
                container(window),
                |bound, (trigger, handler)| match *trigger {
                    Trigger::Command(command) => bound.on_command(command, handler.clone()),
                    Trigger::Chord(key, modifiers) => {
                        bound.on_chord(key, modifiers, handler.clone())
                    }
                },
            )
            .fill()
            .build()
    }
}

impl std::fmt::Debug for Bindings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list()
            .entries(self.bound.iter().map(|(trigger, _)| trigger))
            .finish()
    }
}
