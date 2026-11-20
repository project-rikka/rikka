use rikka_symbols::MaterialSymbol;
use rikka_text::IconSpec;
use rikka_types::{Key, Point, Rect, RoundedRect, Size};
use rikka_ui::text::{Label, TextRun};
use rikka_ui::{
    Action, Behavior, Event, EventCtx, IntoHandler, Layout, Length, PaintCtx, Style, Widget, motion,
};

use crate::control::ControlSize;
use crate::controls::press::Press;
use crate::controls::ripple::Ripple;
use crate::lists::collection::KeyboardWalk;
use crate::rows;
use crate::text::field::{self, FieldMood};

const PAGE: usize = 10;

const CHEVRON_TRAVEL: f32 = 0.4;

const TURN: &str = "dropdown.turn";

#[must_use]
#[derive(Widget)]
pub struct Dropdown {
    trigger: TriggerProps,
    #[debug]
    open: bool,
    on_toggle: Action<(bool,)>,
    on_select: Action<(usize,)>,
    #[style]
    style: Style,
}

pub fn dropdown(selected: impl Into<Option<usize>>) -> Dropdown {
    Dropdown {
        trigger: TriggerProps::new(selected.into()),
        open: false,
        on_toggle: Action::default(),
        on_select: Action::default(),
        style: Style::default(),
    }
}

impl Dropdown {
    crate::navigation::dropdown::trigger_methods!();

    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    pub fn on_toggle<Marker>(mut self, on_toggle: impl IntoHandler<(bool,), (), Marker>) -> Self {
        self.on_toggle = Action::new(on_toggle);
        self
    }

    pub fn on_select<Marker>(mut self, on_select: impl IntoHandler<(usize,), (), Marker>) -> Self {
        self.on_select = Action::new(on_select);
        self
    }

    fn is_active(&self) -> bool {
        self.on_toggle.active(self.trigger.enabled)
    }

    fn face(&self) -> Trigger<'_> {
        self.trigger.face(self.open, self.is_active())
    }
}

pub(crate) struct TriggerProps {
    pub(crate) selected: Option<usize>,
    pub(crate) options: Vec<Label>,
    pub(crate) placeholder: Option<Label>,
    pub(crate) size: ControlSize,
    pub(crate) enabled: bool,
}

impl TriggerProps {
    pub(crate) fn new(selected: Option<usize>) -> Self {
        Self {
            selected,
            options: Vec::new(),
            placeholder: None,
            size: ControlSize::default(),
            enabled: true,
        }
    }

    pub(crate) fn chosen(&self) -> Option<&Label> {
        self.options.get(self.selected?)
    }

    pub(crate) fn face(&self, open: bool, active: bool) -> Trigger<'_> {
        Trigger {
            size: self.size,
            chosen: self.chosen(),
            placeholder: self.placeholder.as_ref(),
            open,
            disabled: !self.enabled,
            active,
        }
    }

    pub(crate) fn walk(&self, event: &Event, on_select: &Action<(usize,)>, ctx: &mut EventCtx<'_>) {
        let Event::KeyPressed { key, .. } = event else {
            return;
        };
        if !self.enabled || !ctx.focused() {
            return;
        }
        let walked = KeyboardWalk::column(self.options.len(), PAGE);
        let Some(next) = walked.to(*key, self.selected, |_| true) else {
            return;
        };
        ctx.set_handled();
        rows::select(ctx, on_select, self.selected, next);
    }
}

macro_rules! trigger_methods {
    () => {
        pub fn options<L: Into<::rikka_ui::Label>>(
            mut self,
            options: impl IntoIterator<Item = L>,
        ) -> Self {
            self.trigger.options = options.into_iter().map(Into::into).collect();
            self
        }

        pub fn option(mut self, option: impl Into<::rikka_ui::Label>) -> Self {
            self.trigger.options.push(option.into());
            self
        }

        pub fn placeholder(mut self, placeholder: impl Into<::rikka_ui::Label>) -> Self {
            self.trigger.placeholder = Some(placeholder.into());
            self
        }

        pub fn control_size(mut self, size: $crate::ControlSize) -> Self {
            self.trigger.size = size;
            self
        }

        pub fn enabled(mut self, enabled: bool) -> Self {
            self.trigger.enabled = enabled;
            self
        }
    };
}

pub(crate) use trigger_methods;

pub(crate) struct Trigger<'a> {
    pub(crate) size: ControlSize,
    pub(crate) chosen: Option<&'a Label>,
    pub(crate) placeholder: Option<&'a Label>,
    pub(crate) open: bool,
    pub(crate) disabled: bool,
    pub(crate) active: bool,
}

impl Trigger<'_> {
    fn padding(&self) -> f32 {
        self.size.text_inset()
    }

    pub(crate) fn layout(&self, props: &Style) -> Layout {
        Layout::row()
            .size(Length::FILL, Length::px(self.size.height()))
            .fixed()
            .with(props)
    }

    pub(crate) fn paint(&self, ctx: &mut PaintCtx<'_>, state: &mut DropdownState, bounds: Rect) {
        let palette = ctx.theme().palette();
        let lit = self.open || ctx.focused();

        let (radius, font_size) = (self.size.radius(), self.size.font_size(ctx.theme()));
        let mood = FieldMood {
            enabled: !self.disabled,
            attentive: lit,
            hovered: ctx.hovered() && self.active,
            invalid: false,
        };
        field::paint_frame(ctx, bounds, radius, mood);
        state.ripple.paint(
            ctx,
            &state.press,
            RoundedRect::new(bounds, radius),
            self.active,
        );

        let side = self.size.icon_size();
        let center = Point::new(
            bounds.max_x() - self.padding() - side / 2.0,
            bounds.center().y,
        );
        let hint = ctx.theme().dim(palette.text_muted, self.disabled);
        let turn = ctx.animate_curve(TURN, if self.open { 1.0 } else { 0.0 }, motion::MORPH);
        let travel = side * CHEVRON_TRAVEL;
        for (glyph, alpha, offset) in [
            (MaterialSymbol::EXPAND_MORE, 1.0 - turn, turn * travel),
            (MaterialSymbol::EXPAND_LESS, turn, (turn - 1.0) * travel),
        ] {
            if alpha <= 0.0 {
                continue;
            }
            let at = Point::new(center.x, center.y + offset);
            ctx.draw_icon(
                Rect::from_center(at, Size::new(side, side)),
                IconSpec::new(glyph.codepoint(), side),
                hint.scale_alpha(alpha),
            );
        }

        let (label, color) = match self.chosen {
            Some(label) => (label, palette.text),
            None => match self.placeholder {
                Some(placeholder) => (placeholder, palette.text_muted),
                None => return,
            },
        };
        let color = ctx.theme().dim(color, self.disabled);
        let x = bounds.x() + self.padding();
        let width = center.x - side / 2.0 - self.size.gap() - x;
        let run = TextRun::new(label.clone(), font_size).no_wrap();
        rows::draw_cell(ctx, bounds, x, width, run, color);
    }
}

#[derive(Debug, Default)]
#[doc(hidden)]
pub struct DropdownState {
    pub(crate) press: Press,
    ripple: Ripple,
}

impl Widget for Dropdown {
    type State = DropdownState;

    fn layout(&self) -> Layout {
        self.face().layout(&self.style)
    }

    fn paint(&self, state: &mut DropdownState, ctx: &mut PaintCtx<'_>) {
        let bounds = ctx.bounds();
        self.face().paint(ctx, state, bounds);
    }

    fn event(&self, state: &mut DropdownState, event: &Event, ctx: &mut EventCtx<'_>) {
        if state.press.update(event, self.is_active(), ctx) {
            ctx.act(&self.on_toggle, (!self.open,));
            return;
        }
        if let Event::KeyPressed {
            key: Key::Escape, ..
        } = event
        {
            if ctx.focused() && self.open && self.on_toggle.active(self.trigger.enabled) {
                ctx.set_handled();
                ctx.act(&self.on_toggle, (false,));
            }
            return;
        }
        self.trigger.walk(event, &self.on_select, ctx);
    }

    fn behavior(&self, _state: &Self::State) -> Behavior {
        Behavior::TRACKS_HOVER
            | Behavior::FOCUSABLE.when(
                self.on_toggle.active(self.trigger.enabled)
                    || self.on_select.active(self.trigger.enabled),
            )
    }
}
