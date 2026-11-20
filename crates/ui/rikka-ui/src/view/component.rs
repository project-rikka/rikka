use std::any::{Any, TypeId, type_name};

use crate::view::{Element, StateAccess, View, ViewFn};

pub trait ViewWith<'a, C: 'a, P: 'a>: 'static {
    fn build(&self, content: &'a C, props: &'a P) -> Element;
}

impl<'a, C: 'a, P: 'a, F, V> ViewWith<'a, C, P> for F
where
    F: Fn(&'a C, &'a P) -> V + 'static,
    V: View,
{
    fn build(&self, content: &'a C, props: &'a P) -> Element {
        self(content, props).build()
    }
}

pub fn component<S, C, F>(select: fn(&S) -> &C, view: F) -> Element
where
    S: 'static,
    C: 'static,
    F: for<'a> ViewFn<'a, C>,
{
    const { assert!(size_of::<F>() == 0, "a component's view captures nothing") };
    Element::sourced(Box::new(Component {
        select,
        props: (),
        viewer: Bare(view),
    }))
}

pub fn component_with<S, C, P, F>(select: fn(&S) -> &C, props: P, view: F) -> Element
where
    S: 'static,
    C: 'static,
    P: PartialEq + 'static,
    F: for<'a> ViewWith<'a, C, P>,
{
    const { assert!(size_of::<F>() == 0, "a component's view captures nothing") };
    Element::sourced(Box::new(Component {
        select,
        props,
        viewer: With(view),
    }))
}

pub(crate) trait Source: 'static {
    fn identity(&self) -> TypeId;

    fn same(&self, other: &dyn Source) -> bool;

    fn view(&self, reach: &mut StateAccess<'_>) -> Element;

    fn as_any(&self) -> &dyn Any;

    fn name(&self) -> &'static str;
}

trait Viewer<C, P>: 'static {
    fn view(&self, content: &C, props: &P) -> Element;
}

struct Bare<F>(F);

impl<C: 'static, F: for<'a> ViewFn<'a, C>> Viewer<C, ()> for Bare<F> {
    fn view(&self, content: &C, (): &()) -> Element {
        self.0.build(content)
    }
}

struct With<F>(F);

impl<C: 'static, P: 'static, F: for<'a> ViewWith<'a, C, P>> Viewer<C, P> for With<F> {
    fn view(&self, content: &C, props: &P) -> Element {
        self.0.build(content, props)
    }
}

struct Component<S: 'static, C: 'static, P, W> {
    select: fn(&S) -> &C,
    props: P,
    viewer: W,
}

impl<S, C, P, W> Source for Component<S, C, P, W>
where
    S: 'static,
    C: 'static,
    P: PartialEq + 'static,
    W: Viewer<C, P>,
{
    fn identity(&self) -> TypeId {
        TypeId::of::<Self>()
    }

    fn same(&self, other: &dyn Source) -> bool {
        other.as_any().downcast_ref::<Self>().is_some_and(|other| {
            std::ptr::fn_addr_eq(self.select, other.select) && self.props == other.props
        })
    }

    fn view(&self, reach: &mut StateAccess<'_>) -> Element {
        match reach.find::<S>() {
            Some(source) => self.viewer.view((self.select)(source), &self.props),
            None => Element::empty(),
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn name(&self) -> &'static str {
        type_name::<W>()
    }
}
