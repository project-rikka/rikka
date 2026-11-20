use rikka_symbols::MaterialSymbol;
use rikka_text::IconSpec;
use rikka_theme::caption;
use rikka_types::{Point, Rect};
use rikka_ui::{
    Behavior, Event, EventCtx, Layout, Length, PaintCtx, Quad, Widget, WindowPart, WindowRequest,
    font, motion,
};

use crate::controls::press::Press;
use crate::controls::ripple::{engaged, state_layer};

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub(crate) enum CaptionKind {
    Minimize,
    Maximize,
    Restore,
    Close,
}

impl CaptionKind {
    fn symbol(self) -> MaterialSymbol {
        match self {
            Self::Minimize => MaterialSymbol::REMOVE,
            Self::Maximize => MaterialSymbol::CROP_SQUARE,
            Self::Restore => MaterialSymbol::FILTER_NONE,
            Self::Close => MaterialSymbol::CLOSE,
        }
    }

    fn request(self) -> WindowRequest {
        match self {
            Self::Minimize => WindowRequest::Minimize,
            Self::Maximize | Self::Restore => WindowRequest::ToggleMaximize,
            Self::Close => WindowRequest::AskToClose,
        }
    }

    fn part(self) -> WindowPart {
        match self {
            Self::Minimize => WindowPart::Minimize,
            Self::Maximize | Self::Restore => WindowPart::Maximize,
            Self::Close => WindowPart::Close,
        }
    }

    fn destructive(self) -> bool {
        matches!(self, Self::Close)
    }
}

#[derive(Widget)]
pub(crate) struct CaptionButton {
    #[debug]
    caption: CaptionKind,
}

pub(crate) fn caption_button(caption: CaptionKind) -> CaptionButton {
    CaptionButton { caption }
}

#[derive(Debug, Default)]
pub(crate) struct CaptionButtonState {
    press: Press,
}

impl Widget for CaptionButton {
    type State = CaptionButtonState;

    fn layout(&self) -> Layout {
        Layout::row()
            .width(Length::px(caption::BUTTON_WIDTH))
            .fixed()
    }

    fn paint(&self, state: &mut CaptionButtonState, ctx: &mut PaintCtx<'_>) {
        let resting = ctx.env().text_color;
        let down = state.press.is_down();
        let engaged = engaged(ctx, true, down);
        let (fill, glyph) = if self.caption.destructive() {
            let danger = ctx.theme().palette().danger;
            let fill = engaged.then(|| {
                if down {
                    ctx.theme().pressed(danger)
                } else {
                    danger
                }
            });
            let glyph = if engaged {
                ctx.theme().palette().on_danger
            } else {
                resting
            };
            (fill, glyph)
        } else {
            (state_layer(ctx, true, down), resting)
        };

        let glyph = ctx.animate_curve("glyph", glyph, motion::HOVER);
        if let Some(fill) = ctx.fade("fill", fill, motion::HOVER) {
            ctx.draw_quad(Quad::new(ctx.bounds()).fill(fill));
        }
        ctx.draw_icon(
            ctx.bounds(),
            IconSpec::new(self.caption.symbol().codepoint(), caption::GLYPH).weight(font::LIGHT),
            glyph,
        );
    }

    fn event(&self, state: &mut CaptionButtonState, event: &Event, ctx: &mut EventCtx<'_>) {
        if matches!(event, Event::PointerPressed { .. }) {
            ctx.keep_focus();
        }
        if state.press.update(event, true, ctx) {
            ctx.request_window(self.caption.request());
        }
    }

    fn behavior(&self, _state: &Self::State) -> Behavior {
        Behavior::TRACKS_HOVER
    }

    fn window_part(&self, _bounds: Rect, _point: Point) -> Option<WindowPart> {
        Some(self.caption.part())
    }

    fn same(&self, other: &Self) -> bool {
        self.caption == other.caption
    }
}
