use std::rc::Rc;

use rikka_animation::Motion;
use rikka_types::{Color, Point};

use crate::motion::Transition;
use crate::text::{Label, text};
use crate::view::{Element, Tooltip};

pub trait View {
    fn build(self) -> Element;

    fn any(self) -> Element
    where
        Self: Sized,
    {
        self.build()
    }

    fn opacity(self, opacity: f32) -> Element
    where
        Self: Sized,
    {
        visually(self, |visual| {
            visual.opacity = Some(opacity.clamp(0.0, 1.0))
        })
    }

    fn scale(self, factor: f32) -> Element
    where
        Self: Sized,
    {
        visually(self, |visual| visual.scale = Some(factor.max(0.0)))
    }

    fn offset(self, x: f32, y: f32) -> Element
    where
        Self: Sized,
    {
        visually(self, |visual| visual.offset = Some(Point::new(x, y)))
    }

    fn background(self, color: Color) -> Element
    where
        Self: Sized,
    {
        visually(self, |visual| visual.background = Some(color))
    }

    fn foreground(self, color: Color) -> Element
    where
        Self: Sized,
    {
        visually(self, |visual| visual.foreground = Some(color))
    }

    fn animate(self, motion: Motion) -> Element
    where
        Self: Sized,
    {
        visually(self, |visual| visual.animate = Some(motion))
    }

    fn animated(self) -> Element
    where
        Self: Sized,
    {
        self.animate(Motion::default())
    }

    fn clipped(self) -> Element
    where
        Self: Sized,
    {
        visually(self, |visual| visual.clipped = true)
    }

    fn transition(self, transition: Transition) -> Element
    where
        Self: Sized,
    {
        visually(self, |visual| visual.transition = Some(transition))
    }

    fn tooltip(self, tooltip: impl Into<Tooltip>) -> Element
    where
        Self: Sized,
    {
        let mut element = self.build();
        let tooltip = tooltip.into();
        element.tooltip = (!tooltip.is_blank()).then(|| Rc::new(tooltip));
        element
    }
}

pub trait ViewFn<'a, S: 'a>: 'static {
    fn build(&self, state: &'a S) -> Element;
}

impl<'a, S: 'a, F, V> ViewFn<'a, S> for F
where
    F: Fn(&'a S) -> V + 'static,
    V: View,
{
    fn build(&self, state: &'a S) -> Element {
        self(state).build()
    }
}

fn visually(view: impl View, apply: impl FnOnce(&mut crate::view::Visual)) -> Element {
    let mut element = view.build();
    apply(element.visual_mut());
    element
}

impl View for Element {
    fn build(self) -> Element {
        self
    }
}

impl<V: View> View for Option<V> {
    fn build(self) -> Element {
        self.map_or_else(Element::empty, View::build)
    }
}

impl View for &str {
    fn build(self) -> Element {
        match self.is_empty() {
            true => Element::empty(),
            false => text(self).build(),
        }
    }
}

impl View for String {
    fn build(self) -> Element {
        match self.is_empty() {
            true => Element::empty(),
            false => text(self).build(),
        }
    }
}

impl View for Label {
    fn build(self) -> Element {
        match self.is_empty() {
            true => Element::empty(),
            false => text(self).build(),
        }
    }
}
