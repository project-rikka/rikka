use std::any::TypeId;
use std::rc::Rc;

use crate::style::Style;
use crate::view::{Anchor, Element, Env, Scoped, Source, Visual};

pub(crate) struct ComponentSlot {
    pub(crate) identity: TypeId,
    pub(crate) source: Option<Box<dyn Source>>,
    pub(crate) built: u64,
    pub(crate) reads: Vec<u64>,
    pub(crate) outer: Outer,
    pub(crate) env: Env,
    pub(crate) dirty: bool,
}

impl ComponentSlot {
    pub(crate) fn keeps(&self, element: &Element, env: &Env) -> bool {
        let same = match (&self.source, &element.source) {
            (Some(component), Some(new)) => component.same(&**new),
            _ => false,
        };
        same && !self.dirty && self.env == *env && self.outer.matches(element)
    }

    pub(crate) fn current_element(&mut self) -> Option<(Element, Env)> {
        let source = self.source.take()?;
        Some((self.outer.element(source), self.env))
    }

    pub(crate) fn name(&self) -> &'static str {
        self.source
            .as_ref()
            .map_or("component", |source| source.name())
    }
}

pub(crate) struct Outer {
    key: Option<u64>,
    branch: Option<u64>,
    anchor: Option<Anchor>,
    style: Option<Box<Style>>,
    visual: Option<Box<Visual>>,
    scopes: Vec<Rc<Scoped>>,
}

impl Outer {
    pub(crate) fn from_element(element: &Element) -> Outer {
        Outer {
            key: element.key,
            branch: element.branch,
            anchor: element.anchor,
            style: element.style.clone(),
            visual: element.visual.clone(),
            scopes: element.scopes.clone(),
        }
    }

    fn matches(&self, element: &Element) -> bool {
        self.key == element.key
            && self.branch == element.branch
            && self.anchor == element.anchor
            && self.style == element.style
            && self.visual == element.visual
            && self.scopes.len() == element.scopes.len()
            && std::iter::zip(&self.scopes, &element.scopes)
                .all(|(component, new)| component.same(new))
    }

    fn element(&self, source: Box<dyn Source>) -> Element {
        let mut element = Element::sourced(source);
        element.key = self.key;
        element.branch = self.branch;
        element.anchor = self.anchor;
        element.style = self.style.clone();
        element.visual = self.visual.clone();
        element.scopes = self.scopes.clone();
        element
    }
}
