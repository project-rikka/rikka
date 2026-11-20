mod edits;
mod gutter;

use std::ops::RangeInclusive;

use rikka_text::{Family, Wrap};
use rikka_types::{Color, CursorShape, Key, Modifiers, Point, Rect, Size};

use crate::text::editing::{Editing, EditingState, Options, Room, TAB_WIDTH};
use rikka_ui::scroll::Place;
use rikka_ui::text::{Label, TextDocument, TextRun};
use rikka_ui::{
    Action, Behavior, Env, Event, EventCtx, IntoHandler, Layout, Length, MeasureSpec, PaintCtx,
    Style, Widget,
};

pub use edits::{Edit, Edits};

use gutter::{Digit, Numbers, digit_width};

pub(crate) const PADDING: f32 = 8.0;
pub(crate) const TEXT_INSET: f32 = 12.0;
pub(crate) const GUTTER_INSET: f32 = 12.0;
pub(crate) const GUTTER_GAP: f32 = 16.0;

const HEIGHT_TOLERANCE: f32 = 1.0;

#[must_use]
#[derive(Widget)]
pub struct TextEditor {
    editing: Editing,
    gutter: bool,
    edits: Edits,
    fits_text: Option<RangeInclusive<f32>>,
    autofocus: bool,
    color: Option<Color>,
    font_size: Option<f32>,
    line_height: Option<f32>,
    family: Option<Family>,
    #[style]
    style: Style,
}

pub fn text_editor(document: &TextDocument) -> TextEditor {
    TextEditor {
        editing: Editing::new(document, Options::editor()),
        gutter: true,
        edits: Edits::default(),
        fits_text: None,
        autofocus: false,
        color: None,
        font_size: None,
        line_height: None,
        family: None,
        style: Style {
            default_grow: Some(1.0),
            ..Style::default()
        },
    }
}

impl TextEditor {
    pub fn placeholder(mut self, placeholder: impl Into<Label>) -> Self {
        self.editing.placeholder = Some(placeholder.into());
        self
    }

    pub fn on_change<Marker>(mut self, on_change: impl IntoHandler<(), (), Marker>) -> Self {
        self.editing.on_change = Action::new(on_change);
        self
    }

    pub fn on_submit<Marker>(mut self, on_submit: impl IntoHandler<(), (), Marker>) -> Self {
        self.editing.on_submit = Action::new(on_submit);
        self
    }

    pub fn on_blur<Marker>(mut self, on_blur: impl IntoHandler<(), (), Marker>) -> Self {
        self.editing.on_blur = Action::new(on_blur);
        self
    }

    pub fn on_paste<Marker>(
        mut self,
        on_paste: impl IntoHandler<(Vec<String>,), bool, Marker>,
    ) -> Self {
        self.editing.on_paste = Action::new(on_paste);
        self
    }

    pub fn on_key<Marker>(
        mut self,
        on_key: impl IntoHandler<(Key, Modifiers), bool, Marker>,
    ) -> Self {
        self.editing.on_key = Action::new(on_key);
        self
    }

    pub fn read_only(mut self, read_only: bool) -> Self {
        self.editing.options.editable = !read_only;
        self
    }

    pub fn wrap(mut self, wrap: bool) -> Self {
        self.editing.options.wrap = wrap;
        self
    }

    pub fn code(mut self, code: bool) -> Self {
        self.editing.options.code = code;
        self
    }

    pub fn gutter(mut self, gutter: bool) -> Self {
        self.gutter = gutter;
        self
    }

    pub fn edits(mut self, edits: Edits) -> Self {
        self.edits = edits;
        self
    }

    pub fn fits_text(mut self, heights: RangeInclusive<f32>) -> Self {
        self.fits_text = Some(heights);
        self
    }

    pub fn search(mut self, search: &rikka_ui::search::Search) -> Self {
        self.editing.search = Some(search.clone());
        self
    }

    pub fn place(mut self, place: &Place) -> Self {
        self.editing.place = Some(place.clone());
        self
    }

    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    pub fn font_size(mut self, size: f32) -> Self {
        self.font_size = Some(size);
        self
    }

    pub fn line_height(mut self, line_height: f32) -> Self {
        self.line_height = Some(line_height);
        self
    }

    pub fn family(mut self, family: Family) -> Self {
        self.family = Some(family);
        self
    }

    pub fn language(mut self, language: impl Into<Option<rikka_syntax::Language>>) -> Self {
        self.editing.language = language.into();
        self
    }

    pub fn autofocus(mut self, autofocus: bool) -> Self {
        self.autofocus = autofocus;
        self
    }

    fn editable(&self) -> bool {
        self.editing.editable()
    }

    fn template(&self, env: &Env) -> TextRun {
        self.template_over(None, env)
    }

    fn template_over(&self, previous: Option<&TextRun>, env: &Env) -> TextRun {
        let size = self.font_size.unwrap_or(env.font_size);
        let line_height = self.line_height.unwrap_or((size * 1.5).round());
        let family = self.family.as_ref().unwrap_or(&rikka_ui::Family::Monospace);
        let wrap = match self.editing.wraps() {
            true => Wrap::WordOrGlyph,
            false => Wrap::None,
        };
        if let Some(previous) = previous.filter(|previous| {
            previous.size == size
                && previous.line_height == line_height
                && previous.family == *family
                && previous.wrap == wrap
        }) {
            return previous.clone();
        }
        let mut run = TextRun::new("", size);
        run.line_height = line_height;
        run.family = family.clone();
        run.wrap = wrap;
        run.tab_width = TAB_WIDTH;
        run
    }

    fn gutter_width(&self, digit: f32) -> f32 {
        match self.gutter {
            true => {
                GUTTER_INSET
                    + digits_in(self.editing.document.line_count()) as f32 * digit
                    + GUTTER_GAP
            }
            false => TEXT_INSET,
        }
    }

    fn side_room(&self, template: &TextRun) -> f32 {
        let digit = match self.gutter {
            true => digit_width(template),
            false => 0.0,
        };
        self.gutter_width(digit) + TEXT_INSET
    }

    fn refit(&self, state: &mut TextEditorState, ctx: &mut PaintCtx<'_>) {
        let Some(heights) = self.fits_text.as_ref() else {
            return;
        };
        let bounds = state.editing.frame.bounds;
        let fitted_height = state
            .editing
            .frame
            .content_height
            .clamp(*heights.start(), *heights.end());
        if (bounds.height() - fitted_height).abs() < HEIGHT_TOLERANCE {
            state.refused = None;
            return;
        }
        let key = (bounds.width().to_bits(), fitted_height.to_bits());
        if state.refused == Some(key) {
            return;
        }
        state.refused = Some(key);
        ctx.request_layout();
    }
}

fn digits_in(lines: usize) -> usize {
    lines.max(1).ilog10() as usize + 1
}

#[derive(Debug, Default)]
#[doc(hidden)]
pub struct TextEditorState {
    pub(crate) editing: EditingState,
    pub(crate) digit: Digit,
    pub(crate) numbers: Numbers,
    pub(crate) template: Option<TextRun>,
    pub(crate) refused: Option<(u32, u32)>,
}

impl Widget for TextEditor {
    type State = TextEditorState;

    fn layout(&self) -> Layout {
        let mut props = self.style;
        let mut layout = Layout::row()
            .width(Length::FILL)
            .min_width(Length::px(0.0))
            .min_height(Length::px(0.0));
        if let Some(heights) = &self.fits_text {
            props.default_grow = None;
            layout = layout
                .min_height(Length::px(*heights.start()))
                .max_height(Length::px(*heights.end()));
        }
        layout.with(&props)
    }

    fn measure(&self) -> Option<MeasureSpec> {
        let env = &rikka_ui::env();
        let heights = self.fits_text.as_ref()?;
        let template = self.template(env);
        let inset = Size::new(self.side_room(&template), PADDING * 2.0);
        Some(MeasureSpec::document(
            &self.editing.document,
            template,
            inset,
            heights.clone(),
        ))
    }

    fn behavior(&self, _state: &Self::State) -> Behavior {
        Behavior::CLIPS
            | Behavior::TRACKS_HOVER
            | Behavior::FOCUSABLE
            | Behavior::AUTOFOCUS.when(self.autofocus)
            | Behavior::TAKES_TAB.when(self.editable())
    }

    fn paint(&self, state: &mut TextEditorState, ctx: &mut PaintCtx<'_>) {
        let template = self.template_over(state.template.as_ref(), ctx.env());
        state.template = Some(template.clone());
        let gutter = self.gutter_width(state.digit.width(&template));
        let palette = ctx.theme().palette();
        let room = Room {
            bounds: ctx.bounds(),
            left: gutter,
            right: TEXT_INSET,
            top: PADDING,
            ink: self.color.unwrap_or(ctx.env().text_color),
            hint: palette.text_muted,
        };
        self.editing.paint(&mut state.editing, ctx, room, &template);
        self.draw_gutter(state, ctx, gutter, palette, &template);
        self.refit(state, ctx);
    }

    fn event(&self, state: &mut TextEditorState, event: &Event, ctx: &mut EventCtx<'_>) {
        let revision = self.editing.document.revision();
        self.editing.event(&mut state.editing, event, ctx);
        if self.fits_text.is_some() && self.editing.document.revision() != revision {
            ctx.request_layout();
        }
    }

    fn cursor(&self, _state: &Self::State, bounds: Rect, point: Point) -> Option<CursorShape> {
        bounds.contains(point).then_some(CursorShape::Text)
    }

    fn caret(&self, state: &TextEditorState) -> Option<Rect> {
        self.editable()
            .then(|| state.editing.caret_within())
            .flatten()
    }

    fn relocated(&self, state: &mut TextEditorState, by: Point) {
        state.editing.relocated(by);
    }

    fn same(&self, other: &Self) -> bool {
        self.editing.looks_like(&other.editing)
            && self.gutter == other.gutter
            && self.edits == other.edits
            && self.fits_text == other.fits_text
            && self.autofocus == other.autofocus
            && self.color == other.color
            && self.font_size == other.font_size
            && self.line_height == other.line_height
            && self.family == other.family
            && self.style == other.style
    }
}
