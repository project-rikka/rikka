use std::hash::Hash;

use rikka_types::{Key, Modifiers};
use rikka_ui::text::{TextStyled, text};
use rikka_ui::{Align, Element, Handler, IntoHandler, Label, Space, Styled, View};

use crate::blocks::flex::{column_of, row_of};
use crate::blocks::sheet::{Sheet, sheet};
use crate::blocks::spacer::spacer;
use crate::controls::button::button;

const PADDING: f32 = 20.0;

type Placing = Box<dyn FnOnce(Sheet) -> Sheet>;

struct Choice {
    label: Label,
    handler: Handler,
}

#[must_use]
pub struct Dialog {
    title: Label,
    summary: Option<Label>,
    body: Element,
    aside: Vec<Element>,
    actions: Vec<Element>,
    cancel: Option<Choice>,
    accept: Option<Choice>,
    ready: bool,
    dismiss: Option<Handler>,
    chords: Vec<(Key, Modifiers, Handler)>,
    width: Option<f32>,
    placing: Vec<Placing>,
}

pub fn dialog(title: impl Into<Label>, body: impl View) -> Dialog {
    Dialog {
        title: title.into(),
        summary: None,
        body: body.build(),
        aside: Vec::new(),
        actions: Vec::new(),
        cancel: None,
        accept: None,
        ready: true,
        dismiss: None,
        chords: Vec::new(),
        width: None,
        placing: Vec::new(),
    }
}

impl Dialog {
    pub fn summary(mut self, summary: impl Into<Label>) -> Self {
        self.summary = Some(summary.into());
        self
    }

    pub fn aside(mut self, element: impl View) -> Self {
        self.aside.push(element.build());
        self
    }

    pub fn action(mut self, element: impl View) -> Self {
        self.actions.push(element.build());
        self
    }

    pub fn cancel<Marker>(
        mut self,
        label: impl Into<Label>,
        handler: impl IntoHandler<(), (), Marker>,
    ) -> Self {
        self.cancel = Some(Choice {
            label: label.into(),
            handler: handler.into_handler(),
        });
        self
    }

    pub fn accept<Marker>(
        mut self,
        label: impl Into<Label>,
        handler: impl IntoHandler<(), (), Marker>,
    ) -> Self {
        self.accept = Some(Choice {
            label: label.into(),
            handler: handler.into_handler(),
        });
        self
    }

    pub fn ready(mut self, ready: bool) -> Self {
        self.ready = ready;
        self
    }

    pub fn on_dismiss<Marker>(mut self, dismiss: impl IntoHandler<(), (), Marker>) -> Self {
        self.dismiss = Some(dismiss.into_handler());
        self
    }

    pub fn on_chord<Marker>(
        mut self,
        key: Key,
        modifiers: Modifiers,
        handler: impl IntoHandler<(), (), Marker>,
    ) -> Self {
        self.chords.push((key, modifiers, handler.into_handler()));
        self
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    pub fn keyed(mut self, key: impl Hash + 'static) -> Self {
        self.placing.push(Box::new(move |sheet| sheet.keyed(key)));
        self
    }

    pub fn centered_on(mut self, stage: impl Hash + 'static) -> Self {
        self.placing
            .push(Box::new(move |sheet| sheet.centered_on(stage)));
        self
    }

    pub fn with_scrim(self) -> Vec<Element> {
        self.into_sheet().with_scrim()
    }

    fn into_sheet(self) -> Sheet {
        let mut header = vec![text(self.title).label().semibold().build()];
        if let Some(summary) = self.summary {
            header.push(text(summary).caption().muted().build());
        }
        let mut footer = self.aside;
        footer.push(spacer().grow(1.0).build());
        if let Some(cancel) = &self.cancel {
            footer.push(
                button(text(cancel.label.clone()))
                    .ghost()
                    .on_press(cancel.handler.clone())
                    .build(),
            );
        }
        footer.extend(self.actions);
        if let Some(accept) = &self.accept {
            footer.push(
                button(text(accept.label.clone()))
                    .primary()
                    .enabled(self.ready)
                    .on_press(accept.handler.clone())
                    .build(),
            );
        }
        let mut content = header;
        content.push(self.body);
        content.push(
            row_of(footer)
                .gap(Space::SNUG)
                .align_items(Align::Center)
                .fill_width()
                .build(),
        );
        let mut framed = sheet(
            column_of(content)
                .gap(Space::ROOMY)
                .padding(PADDING)
                .fill_width(),
        );
        if let Some(width) = self.width {
            framed = framed.width(width);
        }
        if let Some(dismiss) = self.dismiss.or(self.cancel.map(|cancel| cancel.handler)) {
            framed = framed.on_dismiss(dismiss);
        }
        if let Some(accept) = self.accept.filter(|_| self.ready) {
            framed = framed.on_chord(Key::Enter, Modifiers::NONE, accept.handler);
        }
        framed = self
            .chords
            .into_iter()
            .fold(framed, |framed, (key, modifiers, handler)| {
                framed.on_chord(key, modifiers, handler)
            });
        self.placing
            .into_iter()
            .fold(framed, |framed, place| place(framed))
    }
}

impl View for Dialog {
    fn build(self) -> Element {
        self.into_sheet().build()
    }
}

impl std::fmt::Debug for Dialog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Dialog")
            .field("title", &self.title)
            .field("ready", &self.ready)
            .finish_non_exhaustive()
    }
}
