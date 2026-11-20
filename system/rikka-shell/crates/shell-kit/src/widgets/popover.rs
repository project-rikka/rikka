use rikka_types::{Key, Modifiers, Point, Rect};
use rikka_ui::{Anchor, Element, Handler, IntoHandler, Length, Styled, View, is_exiting, pop};
use rikka_widgets::column;

use crate::AimTarget;
use crate::widgets::cursor::{mark, whole};
use crate::widgets::{CURSOR, cursor, scrim, sheet};

const CURSOR_GAP: f32 = 4.0;

const MARK_GAP: f32 = 6.0;

const WIDEST: f32 = 92.0;

#[derive(Debug)]
enum Origin {
    Pointer(Point),
    Mark(Rect),
    Centre,
}

#[derive(Debug)]
pub struct Popover {
    panel: Element,
    width: f32,
    standing: Origin,
    keeps_tab: bool,
    on_key: Option<Handler<(Key, Modifiers), bool>>,
    on_dismiss: Handler,
    on_reopen: Option<Handler<(Point,)>>,
}

#[must_use]
pub fn popover<Marker>(panel: impl View, on_dismiss: impl IntoHandler<(), (), Marker>) -> Popover {
    Popover {
        panel: panel.build(),
        width: 0.0,
        standing: Origin::Centre,
        keeps_tab: false,
        on_key: None,
        on_dismiss: on_dismiss.into_handler(),
        on_reopen: None,
    }
}

impl Popover {
    #[must_use]
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    #[must_use]
    pub fn at(mut self, at: Point) -> Self {
        self.standing = Origin::Pointer(at);
        self
    }

    #[must_use]
    pub fn beside(mut self, sighted: Option<AimTarget>) -> Self {
        self.standing = match sighted {
            Some(AimTarget::Caret(rect)) => Origin::Mark(rect),
            Some(AimTarget::Pointer(at)) => Origin::Pointer(at),
            None => Origin::Centre,
        };
        self
    }

    #[must_use]
    pub fn keeps_tab(mut self) -> Self {
        self.keeps_tab = true;
        self
    }

    #[must_use]
    pub fn on_key<Marker>(
        mut self,
        on_key: impl IntoHandler<(Key, Modifiers), bool, Marker>,
    ) -> Self {
        self.on_key = Some(on_key.into_handler());
        self
    }

    #[must_use]
    pub fn on_reopen<Marker>(mut self, on_reopen: impl IntoHandler<(Point,), (), Marker>) -> Self {
        self.on_reopen = Some(on_reopen.into_handler());
        self
    }
}

impl View for Popover {
    fn build(self) -> Element {
        let Popover {
            panel,
            width,
            standing,
            keeps_tab,
            on_key,
            on_dismiss,
            on_reopen,
        } = self;

        let (target, anchor) = match standing {
            Origin::Pointer(at) => (
                cursor(at).build(),
                Anchor::below(CURSOR).offset(CURSOR_GAP, CURSOR_GAP),
            ),
            Origin::Mark(rect) => (
                mark(rect).build(),
                Anchor::below(CURSOR).offset(0.0, MARK_GAP),
            ),
            Origin::Centre => (whole().build(), Anchor::centered_on(CURSOR)),
        };

        let panel = (!is_exiting()).then(|| {
            let on_key = on_key.unwrap_or_else(|| Handler::new(|_: Key, _: Modifiers| false));
            let mut sheet = sheet(panel, on_key);
            if keeps_tab {
                sheet = sheet.keeps_tab();
            }
            sheet
                .build()
                .width(width)
                .max_width(Length::portion(WIDEST / 100.0))
                .transition(pop())
                .anchored(anchor)
        });

        let scrim = scrim(
            column! {
                target;
                panel
            }
            .fill(),
            on_dismiss,
        );
        match on_reopen {
            Some(on_reopen) => scrim.on_secondary_press(on_reopen).build(),
            None => scrim.build(),
        }
    }
}
