use std::any::{Any, TypeId};

use rikka_types::{CursorShape, Point, Rect, Size};

use crate::ambient::{self, ThemeHandle};
use crate::input::{Event, EventCtx, RawEventCtx};
use crate::paint::{MeasureSpec, PaintCtx};
use crate::scroll::Reveal;
use crate::style::Layout;
use crate::view::{Arranged, Behavior, Element, Env, Tooltip};
use crate::window::WindowPart;

pub use rikka_ui_macros::Widget;

pub trait Widget: 'static {
    type State: Default + 'static;

    const ARRANGES: bool = false;

    fn layout(&self) -> Layout {
        Layout::DEFAULT
    }

    fn paint(&self, state: &mut Self::State, ctx: &mut PaintCtx<'_>) {
        let _ = (state, ctx);
    }

    fn behavior(&self, state: &Self::State) -> Behavior {
        let _ = state;
        Behavior::NONE
    }

    fn event(&self, state: &mut Self::State, event: &Event, ctx: &mut EventCtx<'_>) {
        let _ = (state, event, ctx);
    }

    fn build(&self, state: &mut Self::State) -> Vec<Element> {
        let _ = state;
        Vec::new()
    }

    fn keeps_children(&self) -> bool {
        false
    }

    fn freezes_children(&self) -> bool {
        false
    }

    fn settled_height(&self, state: &Self::State) -> Option<f32> {
        let _ = state;
        None
    }

    fn measure(&self) -> Option<MeasureSpec> {
        None
    }

    fn same(&self, other: &Self) -> bool
    where
        Self: Sized,
    {
        let _ = other;
        false
    }

    fn memo(&self) -> Option<u64> {
        None
    }

    fn surroundings(&self, env: &mut Env) {
        let _ = env;
    }

    fn children_offset(&self, state: &Self::State, bounds: Rect, content_size: Size) -> Point {
        let _ = (state, bounds, content_size);
        Point::ZERO
    }

    fn content_length(&self, state: &Self::State) -> Option<f32> {
        let _ = state;
        None
    }

    fn arranged(&self, state: &mut Self::State, arranged: Arranged<'_>) -> bool {
        let _ = (state, arranged);
        false
    }

    fn reveal(
        &self,
        state: &mut Self::State,
        room: Rect,
        content_size: Size,
        rect: Rect,
        reveal: Reveal,
    ) -> bool {
        let _ = (state, room, content_size, rect, reveal);
        false
    }

    fn relocated(&self, state: &mut Self::State, by: Point) {
        let _ = (state, by);
    }

    fn hit_test(&self, bounds: Rect, point: Point) -> bool {
        bounds.contains(point)
    }

    fn hit_area(&self, bounds: Rect) -> Rect {
        bounds
    }

    fn cursor(&self, state: &Self::State, bounds: Rect, point: Point) -> Option<CursorShape> {
        let _ = (state, bounds, point);
        None
    }

    fn window_part(&self, bounds: Rect, point: Point) -> Option<WindowPart> {
        let _ = (bounds, point);
        None
    }

    fn tooltip_at(
        &self,
        state: &Self::State,
        bounds: Rect,
        point: Point,
    ) -> Option<(Tooltip, Rect)> {
        let _ = (state, bounds, point);
        None
    }

    fn caret(&self, state: &Self::State) -> Option<Rect> {
        let _ = state;
        None
    }

    fn debug_name(&self) -> &'static str {
        short_name(std::any::type_name::<Self>())
    }

    fn element(self) -> Element
    where
        Self: Sized,
    {
        Element::new(self)
    }
}

pub(crate) trait AnyWidget: 'static {
    fn widget_type(&self) -> TypeId;
    fn as_any(&self) -> &dyn Any;
    fn same(&self, other: &dyn AnyWidget) -> bool;
    fn debug_name(&self) -> &'static str;
    fn init_state(&self) -> Box<dyn Any>;
    fn behavior(&self, state: &dyn Any) -> Behavior;
    fn memo(&self) -> Option<u64>;
    fn keeps_children(&self) -> bool;
    fn freezes_children(&self) -> bool;
    fn build(&self, state: &mut dyn Any, theme: &ThemeHandle, env: Env) -> Vec<Element>;
    fn layout(&self, theme: &ThemeHandle, env: &Env) -> Layout;
    fn settled_height(&self, state: &dyn Any) -> Option<f32>;
    fn measure(&self, theme: &ThemeHandle, env: &Env) -> Option<MeasureSpec>;
    fn surroundings(&self, theme: &ThemeHandle, env: &mut Env);
    fn paint(&self, state: &mut dyn Any, ctx: &mut PaintCtx<'_>);
    fn event(&self, state: &mut dyn Any, event: &Event, raw: RawEventCtx<'_>);
    fn children_offset(&self, state: &dyn Any, bounds: Rect, content_size: Size) -> Point;
    fn content_length(&self, state: &dyn Any) -> Option<f32>;
    fn arranges(&self) -> bool;
    fn arranged(&self, state: &mut dyn Any, arranged: Arranged<'_>) -> bool;
    fn reveal(
        &self,
        state: &mut dyn Any,
        room: Rect,
        content_size: Size,
        rect: Rect,
        reveal: Reveal,
    ) -> bool;
    fn relocated(&self, state: &mut dyn Any, by: Point);
    fn hit_test(&self, bounds: Rect, point: Point) -> bool;
    fn hit_area(&self, bounds: Rect) -> Rect;
    fn cursor(
        &self,
        state: &dyn Any,
        bounds: Rect,
        point: Point,
        theme: &ThemeHandle,
        env: &Env,
    ) -> Option<CursorShape>;
    fn window_part(&self, bounds: Rect, point: Point) -> Option<WindowPart>;
    fn tooltip_at(
        &self,
        state: &dyn Any,
        bounds: Rect,
        point: Point,
        theme: &ThemeHandle,
        env: &Env,
    ) -> Option<(Tooltip, Rect)>;
    fn caret(&self, state: &dyn Any, theme: &ThemeHandle, env: &Env) -> Option<Rect>;
}

pub(crate) struct TypedWidget<W> {
    pub(crate) widget: W,
}

impl<W> TypedWidget<W> {
    pub(crate) fn new(widget: W) -> Self {
        Self { widget }
    }
}

const KEPT: &str = "widget state type preserved by reconciliation";

impl<W: Widget> AnyWidget for TypedWidget<W> {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn same(&self, other: &dyn AnyWidget) -> bool {
        other
            .as_any()
            .downcast_ref::<TypedWidget<W>>()
            .is_some_and(|other| self.widget.same(&other.widget))
    }

    fn widget_type(&self) -> TypeId {
        TypeId::of::<W>()
    }

    fn debug_name(&self) -> &'static str {
        self.widget.debug_name()
    }

    fn init_state(&self) -> Box<dyn Any> {
        Box::new(W::State::default())
    }

    fn behavior(&self, state: &dyn Any) -> Behavior {
        self.widget
            .behavior(state.downcast_ref::<W::State>().expect(KEPT))
    }

    fn layout(&self, theme: &ThemeHandle, env: &Env) -> Layout {
        ambient::with_env(theme, *env, || self.widget.layout())
    }

    fn settled_height(&self, state: &dyn Any) -> Option<f32> {
        self.widget
            .settled_height(state.downcast_ref::<W::State>().expect(KEPT))
    }

    fn measure(&self, theme: &ThemeHandle, env: &Env) -> Option<MeasureSpec> {
        ambient::with_env(theme, *env, || self.widget.measure())
    }

    fn memo(&self) -> Option<u64> {
        self.widget.memo()
    }

    fn keeps_children(&self) -> bool {
        self.widget.keeps_children()
    }

    fn freezes_children(&self) -> bool {
        self.widget.freezes_children()
    }

    fn build(&self, state: &mut dyn Any, theme: &ThemeHandle, env: Env) -> Vec<Element> {
        let state = state.downcast_mut::<W::State>().expect(KEPT);
        let mut born: Vec<Element> = ambient::with_env(theme, env, || self.widget.build(state));
        born.retain(|child| !child.is_empty());
        if !born.is_empty() {
            crate::stats::built();
        }
        born
    }

    fn surroundings(&self, theme: &ThemeHandle, env: &mut Env) {
        ambient::with_env(theme, *env, || self.widget.surroundings(env));
    }

    fn paint(&self, state: &mut dyn Any, ctx: &mut PaintCtx<'_>) {
        let state = state.downcast_mut::<W::State>().expect(KEPT);
        let (theme, env) = (ctx.theme.clone(), *ctx.env);
        ambient::with_env(&theme, env, || self.widget.paint(state, ctx));
    }

    fn event(&self, state: &mut dyn Any, event: &Event, raw: RawEventCtx<'_>) {
        let state = state.downcast_mut::<W::State>().expect(KEPT);
        let (theme, env) = (raw.theme.clone(), raw.env);
        let mut ctx = EventCtx { raw };
        ambient::with_env(&theme, env, || self.widget.event(state, event, &mut ctx));
    }

    fn children_offset(&self, state: &dyn Any, bounds: Rect, content_size: Size) -> Point {
        let state = state.downcast_ref::<W::State>().expect(KEPT);
        self.widget.children_offset(state, bounds, content_size)
    }

    fn content_length(&self, state: &dyn Any) -> Option<f32> {
        let state = state.downcast_ref::<W::State>().expect(KEPT);
        self.widget.content_length(state)
    }

    fn arranges(&self) -> bool {
        W::ARRANGES
    }

    fn arranged(&self, state: &mut dyn Any, arranged: Arranged<'_>) -> bool {
        let state = state.downcast_mut::<W::State>().expect(KEPT);
        self.widget.arranged(state, arranged)
    }

    fn reveal(
        &self,
        state: &mut dyn Any,
        room: Rect,
        content_size: Size,
        rect: Rect,
        reveal: Reveal,
    ) -> bool {
        let state = state.downcast_mut::<W::State>().expect(KEPT);
        self.widget.reveal(state, room, content_size, rect, reveal)
    }

    fn relocated(&self, state: &mut dyn Any, by: Point) {
        let state = state.downcast_mut::<W::State>().expect(KEPT);
        self.widget.relocated(state, by);
    }

    fn hit_test(&self, bounds: Rect, point: Point) -> bool {
        self.widget.hit_test(bounds, point)
    }

    fn hit_area(&self, bounds: Rect) -> Rect {
        self.widget.hit_area(bounds)
    }

    fn cursor(
        &self,
        state: &dyn Any,
        bounds: Rect,
        point: Point,
        theme: &ThemeHandle,
        env: &Env,
    ) -> Option<CursorShape> {
        let state = state.downcast_ref::<W::State>().expect(KEPT);
        ambient::with_env(theme, *env, || self.widget.cursor(state, bounds, point))
    }

    fn window_part(&self, bounds: Rect, point: Point) -> Option<WindowPart> {
        self.widget.window_part(bounds, point)
    }

    fn tooltip_at(
        &self,
        state: &dyn Any,
        bounds: Rect,
        point: Point,
        theme: &ThemeHandle,
        env: &Env,
    ) -> Option<(Tooltip, Rect)> {
        let state = state.downcast_ref::<W::State>().expect(KEPT);
        ambient::with_env(theme, *env, || self.widget.tooltip_at(state, bounds, point))
    }

    fn caret(&self, state: &dyn Any, theme: &ThemeHandle, env: &Env) -> Option<Rect> {
        let state = state.downcast_ref::<W::State>().expect(KEPT);
        ambient::with_env(theme, *env, || self.widget.caret(state))
    }
}

impl Behavior {
    #[must_use]
    pub fn for_widget<W: Widget>(widget: &W) -> Behavior {
        widget.behavior(&W::State::default())
    }
}

fn short_name(path: &'static str) -> &'static str {
    let bare = path.split('<').next().unwrap_or(path);
    bare.rsplit("::").next().unwrap_or(bare)
}
