use std::hash::Hash;

use rikka_types::{Key, Modifiers};
use rikka_ui::text::{TextStyled, text};
use rikka_ui::{
    Anchor, Corner, Curve, DurationToken, Easing, Element, Handler, IntoHandler, Label, Space,
    Spring, Styled, View, fade, palette, scale, tr,
};

use crate::blocks::container::{Container, container};
use crate::blocks::flex::{column_of, row_of};
use crate::blocks::scrim::scrim;
use crate::blocks::spacer::spacer;
use crate::controls::button::button;

const SHEET_FROM: f32 = 0.88;
const ENTRANCE: Curve = Curve::Spring(Spring::DEFAULT_SPATIAL);
const EXIT: Curve = Curve::Tween {
    easing: Easing::EmphasizedAccel,
    duration: DurationToken::Short4,
};
const PADDING: f32 = 20.0;
const PROMPT_WIDTH: f32 = 420.0;

type Placing = Box<dyn FnOnce(Element) -> Element>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unsaved {
    Save,
    Discard,
    Cancel,
}

#[must_use]
pub struct Sheet {
    panel: Container,
    width: Option<f32>,
    placing: Vec<Placing>,
}

pub fn sheet(content: impl View) -> Sheet {
    Sheet {
        panel: container(content)
            .surface()
            .radius(Corner::Panel)
            .border(1.0, palette().outline)
            .pane()
            .modal(),
        width: None,
        placing: Vec::new(),
    }
}

impl Sheet {
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    pub fn keyed(mut self, key: impl Hash + 'static) -> Self {
        self.placing
            .push(Box::new(move |element| element.keyed(key)));
        self
    }

    pub fn centered_on(mut self, stage: impl Hash + 'static) -> Self {
        self.placing.push(Box::new(move |element| {
            element.anchored(Anchor::centered_on(stage))
        }));
        self
    }

    pub fn on_dismiss<Marker>(self, dismiss: impl IntoHandler<(), (), Marker>) -> Self {
        self.on_chord(Key::Escape, Modifiers::NONE, dismiss)
    }

    pub fn on_chord<Marker>(
        mut self,
        key: Key,
        modifiers: Modifiers,
        handler: impl IntoHandler<(), (), Marker>,
    ) -> Self {
        self.panel = self.panel.on_chord(key, modifiers, handler);
        self
    }

    pub fn with_scrim(self) -> Vec<Element> {
        vec![scrim().transition(fade()), self.build()]
    }
}

impl View for Sheet {
    fn build(self) -> Element {
        let panel = match self.width {
            Some(width) => self.panel.width(width),
            None => self.panel,
        };
        let element = panel
            .transition((fade() + scale(SHEET_FROM)).enter(ENTRANCE).exit(EXIT))
            .build();
        self.placing
            .into_iter()
            .fold(element, |element, place| place(element))
    }
}

impl std::fmt::Debug for Sheet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sheet")
            .field("width", &self.width)
            .finish_non_exhaustive()
    }
}

pub fn unsaved_prompt<Marker>(
    name: &str,
    saving: bool,
    on_answer: impl IntoHandler<(Unsaved,), (), Marker>,
) -> Sheet {
    let answer: Handler<(Unsaved,)> = on_answer.into_handler();
    let answering = |unsaved: Unsaved| answer.clone().map(move |(): ()| (unsaved,));
    prompt(
        tr!("widgets.unsaved.title", name = name),
        tr!("widgets.unsaved.body"),
        vec![
            button(text(tr!("widgets.unsaved.discard")))
                .danger()
                .enabled(!saving)
                .on_press(answering(Unsaved::Discard))
                .build(),
            spacer().grow(1.0).build(),
            button(text(tr!("widgets.unsaved.cancel")))
                .ghost()
                .enabled(!saving)
                .on_press(answering(Unsaved::Cancel))
                .build(),
            button(text(tr!("widgets.unsaved.save")))
                .primary()
                .loading(saving)
                .on_press(answering(Unsaved::Save))
                .build(),
        ],
    )
    .on_dismiss(answering(Unsaved::Cancel))
}

pub fn alert_prompt<Marker>(
    title: impl Into<Label>,
    message: impl Into<Label>,
    on_close: impl IntoHandler<(), (), Marker>,
) -> Sheet {
    let close: Handler = on_close.into_handler();
    prompt(
        title.into(),
        message.into(),
        vec![
            spacer().grow(1.0).build(),
            button(text(tr!("widgets.prompt.ok")))
                .primary()
                .on_press(close.clone())
                .build(),
        ],
    )
    .on_chord(Key::Enter, Modifiers::NONE, close.clone())
    .on_dismiss(close)
}

pub fn confirm_prompt<Marker>(
    title: impl Into<Label>,
    message: impl Into<Label>,
    accept: impl Into<Label>,
    on_answer: impl IntoHandler<(bool,), (), Marker>,
) -> Sheet {
    let answer: Handler<(bool,)> = on_answer.into_handler();
    let answering = |accepted: bool| answer.clone().map(move |(): ()| (accepted,));
    prompt(
        title.into(),
        message.into(),
        vec![
            spacer().grow(1.0).build(),
            button(text(tr!("widgets.prompt.cancel")))
                .ghost()
                .on_press(answering(false))
                .build(),
            button(text(accept.into()))
                .primary()
                .on_press(answering(true))
                .build(),
        ],
    )
    .on_chord(Key::Enter, Modifiers::NONE, answering(true))
    .on_dismiss(answering(false))
}

fn prompt(title: Label, message: Label, buttons: Vec<Element>) -> Sheet {
    let content = column_of(vec![
        text(title).label().semibold().build(),
        text(message).muted().fill_width().build(),
        row_of(buttons)
            .gap(Space::SNUG)
            .centered()
            .fill_width()
            .build(),
    ])
    .gap(Space::ROOMY)
    .padding(PADDING)
    .fill_width();
    sheet(content).width(PROMPT_WIDTH)
}
