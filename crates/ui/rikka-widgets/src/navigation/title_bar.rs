use rikka_theme::{Theme, caption};
use rikka_types::{Color, Point, PointerButton, Rect};
use rikka_ui::text::{Label, TextRun};
use rikka_ui::{
    Align, Behavior, Element, Env, Event, EventCtx, Inset, Justify, Layout, Length, PaintCtx, Quad,
    Style, View, Widget, WindowChrome, WindowPart, WindowRequest, motion,
};

use crate::navigation::caption_button::{CaptionKind, caption_button};
use crate::navigation::panes::SplitRatios;
use crate::rows;

pub(crate) const CAPTION_HEIGHT: f32 = caption::HEIGHT;

const CONTENT_KEY: &str = "rikka:title-bar:content";

const CONTENT_INSET: f32 = 8.0;

const SEPARATOR: f32 = caption::SEPARATOR;

const DRAG_SLOP: f32 = 4.0;

pub struct CaptionBar {
    columns: Vec<(SplitRatios, Element)>,
}

impl CaptionBar {
    pub const HEIGHT: f32 = caption::BAR;

    #[must_use]
    pub fn columns<S: Into<SplitRatios>>(columns: impl IntoIterator<Item = (S, Element)>) -> Self {
        Self {
            columns: columns
                .into_iter()
                .map(|(share, content)| (share.into(), content))
                .collect(),
        }
    }
}

impl From<Element> for CaptionBar {
    fn from(content: Element) -> Self {
        Self::columns([(1.0, content)])
    }
}

impl std::fmt::Debug for CaptionBar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.columns.iter()).finish()
    }
}

pub(crate) struct TitleBar {
    title: Option<Label>,
    focused: bool,
    resizable: bool,
    maximized: bool,
    columns: Vec<(SplitRatios, Element)>,
}

pub(crate) fn title_bar(chrome: &WindowChrome, caption: Option<CaptionBar>) -> TitleBar {
    let columns = caption.map(|bar| bar.columns).unwrap_or_default();
    TitleBar {
        title: columns.is_empty().then(|| chrome.title.clone()),
        focused: chrome.focused,
        resizable: chrome.resizable,
        maximized: chrome.maximized,
        columns,
    }
}

#[derive(Debug, Default)]
pub(crate) struct TitleBarState {
    armed: Option<Point>,
}

impl TitleBar {
    fn captions(&self) -> impl Iterator<Item = CaptionKind> {
        let maximize = match (self.resizable, self.maximized) {
            (false, _) => None,
            (true, false) => Some(CaptionKind::Maximize),
            (true, true) => Some(CaptionKind::Restore),
        };
        [
            Some(CaptionKind::Minimize),
            maximize,
            Some(CaptionKind::Close),
        ]
        .into_iter()
        .flatten()
    }

    fn foreground(&self, theme: &Theme) -> Color {
        let palette = theme.palette();
        match self.focused {
            true => palette.text,
            false => palette.text_muted,
        }
    }

    fn title_area(&self, bounds: Rect, buttons: f32) -> Rect {
        let width = bounds.width() - caption::TITLE_INSET * 2.0 - buttons;
        Rect::new(
            bounds.min_x() + caption::TITLE_INSET,
            bounds.min_y(),
            width.max(0.0),
            bounds.height() - SEPARATOR,
        )
    }
}

impl Widget for TitleBar {
    type State = TitleBarState;

    fn layout(&self) -> Layout {
        Layout::row()
            .align_items(Align::Stretch)
            .justify(Justify::End)
            .size(Length::FILL, Length::px(caption::BAR))
            .padding(Inset::edges(0.0, 0.0, SEPARATOR, 0.0))
            .fixed()
    }

    fn surroundings(&self, env: &mut Env) {
        let theme = rikka_ui::theme();
        env.text_color = self.foreground(&theme);
    }

    fn paint(&self, _: &mut TitleBarState, ctx: &mut PaintCtx<'_>) {
        let theme = ctx.theme();
        let foreground = self.foreground(theme);
        let palette = theme.palette();

        let bounds = ctx.bounds();
        ctx.draw_quad(Quad::new(bounds).fill(palette.background));
        ctx.draw_quad(
            Quad::new(Rect::new(
                bounds.min_x(),
                bounds.max_y() - SEPARATOR,
                bounds.width(),
                SEPARATOR,
            ))
            .fill(palette.outline),
        );

        let Some(title) = self
            .title
            .as_ref()
            .filter(|title| !title.as_str().is_empty())
        else {
            return;
        };
        let color = ctx.animate_curve("title", foreground, motion::COLOR);
        let size = caption::title_size(ctx.theme().typography());
        let run = TextRun::new(title.clone(), size).medium().no_wrap();
        let buttons = self.captions().count() as f32 * caption::BUTTON_WIDTH;
        let area = self.title_area(bounds, buttons);
        rows::draw_cell(ctx, area, area.min_x(), area.width(), run, color);
    }

    fn event(&self, state: &mut TitleBarState, event: &Event, ctx: &mut EventCtx<'_>) {
        if matches!(event, Event::PointerPressed { .. }) {
            ctx.keep_focus();
        }
        match event {
            Event::PointerPressed {
                button: PointerButton::Primary,
                position,
                ..
            } => {
                ctx.set_handled();
                if ctx.presses().is_multiple_of(2) {
                    if self.resizable {
                        ctx.request_window(WindowRequest::ToggleMaximize);
                    }
                    return;
                }
                state.armed = Some(*position);
                ctx.capture_pointer();
            }
            Event::PointerMoved(position) => {
                let Some(origin) = state.armed else {
                    return;
                };
                if position.distance(origin) <= DRAG_SLOP {
                    return;
                }
                state.armed = None;
                ctx.release_pointer();
                ctx.request_window(WindowRequest::Move);
            }
            Event::PointerReleased {
                button: PointerButton::Primary,
                ..
            } => {
                if state.armed.take().is_none() {
                    return;
                }
                ctx.release_pointer();
                ctx.set_handled();
            }
            Event::PointerPressed {
                button: PointerButton::Secondary,
                position,
                ..
            } => {
                ctx.set_handled();
                ctx.request_window(WindowRequest::ShowMenu(*position));
            }
            _ => {}
        }
    }

    fn behavior(&self, _state: &Self::State) -> Behavior {
        Behavior::CLIPS
    }

    fn window_part(&self, _bounds: Rect, _point: Point) -> Option<WindowPart> {
        Some(WindowPart::Caption)
    }
}

impl std::fmt::Debug for TitleBar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TitleBar")
            .field("title", &self.title)
            .field("focused", &self.focused)
            .finish_non_exhaustive()
    }
}

impl View for TitleBar {
    fn build(mut self) -> Element {
        let columns = std::mem::take(&mut self.columns);
        let last = columns.len().saturating_sub(1);
        let mut children: Vec<Element> = columns
            .into_iter()
            .enumerate()
            .map(|(index, (share, content))| caption_cell(content, share, index, index == last))
            .collect();
        children.extend(
            self.captions()
                .map(|caption| caption_button(caption).build().keyed(caption)),
        );
        Element::with_children(self, children)
    }
}

fn caption_cell(content: Element, share: SplitRatios, index: usize, ends: bool) -> Element {
    let leading = if index == 0 { CONTENT_INSET } else { 0.0 };
    let trailing = if ends { CONTENT_INSET } else { 0.0 };
    let named = content.key();
    let layout = CaptionLayout {
        share: (!ends).then_some(share),
        padding: Inset::edges(0.0, trailing, 0.0, leading),
    };
    let column = Element::with_children(layout, vec![content]);
    match named {
        Some(key) => column.keyed(key),
        None => column.keyed((CONTENT_KEY, index)),
    }
}

struct CaptionLayout {
    share: Option<SplitRatios>,
    padding: Inset,
}

impl Widget for CaptionLayout {
    type State = ();

    fn layout(&self) -> Layout {
        let layout = Layout {
            justify: Some(Justify::Center),
            ..Layout::column()
        }
        .padding(self.padding)
        .gap_exact(0.0, 0.0);
        layout.with(&match &self.share {
            Some(share) => Style {
                width: Some(Length::portion(share.value())),
                ..Style::default()
            },
            None => Style {
                grow: Some(1.0),
                ..Style::default()
            },
        })
    }

    fn paint(&self, (): &mut (), _ctx: &mut PaintCtx<'_>) {
        if let Some(share) = &self.share {
            share.watch();
        }
    }

    fn same(&self, other: &Self) -> bool {
        self.share == other.share && self.padding == other.padding
    }

    fn window_part(&self, _bounds: Rect, _point: Point) -> Option<WindowPart> {
        Some(WindowPart::Caption)
    }

    fn debug_name(&self) -> &'static str {
        "CaptionColumn"
    }
}
