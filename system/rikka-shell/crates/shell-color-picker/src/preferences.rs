use std::cell::RefCell;
use std::rc::Rc;

use rikka_color_formats::{Template, template_named};
use rikka_settings::session::color_picker::{
    ACTIVATION, Activation, Button, COPIED_FORMAT, Click, DOMAIN, FORMATS, Format, HEX, SHOW_NAME,
    SHOW_NAME_DEFAULT, built_in_formats,
};
use rikka_types::Choice;
use shell_kit::Shell;

#[derive(Clone, Debug)]
struct Values {
    activation: Activation,
    clicks: [Click; 3],
    copied: String,
    show_name: bool,
    formats: Vec<Format>,
}

#[derive(Clone, Debug)]
pub(crate) struct Preferences(Rc<RefCell<Values>>);

impl Preferences {
    #[must_use]
    pub(crate) fn activation(&self) -> Activation {
        self.0.borrow().activation
    }

    #[must_use]
    pub(super) fn click(&self, button: Button) -> Click {
        self.0.borrow().clicks[slot(button)]
    }

    #[must_use]
    pub(crate) fn copied(&self) -> Template {
        let values = self.0.borrow();
        template_named(&values.formats, &values.copied)
    }

    #[must_use]
    pub(super) fn shows_name(&self) -> bool {
        self.0.borrow().show_name
    }

    #[must_use]
    pub(crate) fn shown(&self) -> Vec<Format> {
        self.0
            .borrow()
            .formats
            .iter()
            .filter(|format| format.shown)
            .cloned()
            .collect()
    }

    fn change(&self, apply: impl FnOnce(&mut Values)) {
        apply(&mut self.0.borrow_mut());
    }
}

fn slot(button: Button) -> usize {
    match button {
        Button::Primary => 0,
        Button::Middle => 1,
        Button::Secondary => 2,
    }
}

pub(crate) fn follow(shell: Shell) -> (Shell, Preferences) {
    let preferences = Preferences(Rc::new(RefCell::new(Values {
        activation: Activation::default(),
        clicks: Button::ALL.map(Button::default_click),
        copied: HEX.to_owned(),
        show_name: SHOW_NAME_DEFAULT,
        formats: built_in_formats(),
    })));

    let shell = shell.follow(DOMAIN, ACTIVATION, {
        let preferences = preferences.clone();
        move |_, named: String| {
            preferences.change(|values| values.activation = Activation::of_slug(&named));
        }
    });
    let shell = Button::ALL.into_iter().fold(shell, |shell, button| {
        let preferences = preferences.clone();
        shell.follow(DOMAIN, button.key(), move |_, named: String| {
            let click = Click::from_slug(&named).unwrap_or(button.default_click());
            preferences.change(|values| values.clicks[slot(button)] = click);
        })
    });
    let shell = shell.follow(DOMAIN, COPIED_FORMAT, {
        let preferences = preferences.clone();
        move |_, named: String| preferences.change(|values| values.copied = named)
    });
    let shell = shell.follow(DOMAIN, SHOW_NAME, {
        let preferences = preferences.clone();
        move |_, on: bool| preferences.change(|values| values.show_name = on)
    });
    let shell = shell.follow(DOMAIN, FORMATS, {
        let preferences = preferences.clone();
        move |_, formats: Vec<Format>| preferences.change(|values| values.formats = formats)
    });

    (shell, preferences)
}
