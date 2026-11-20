use std::any::TypeId;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

use crate::style::{Style, Styled};
use crate::view::{
    Anchor, AnyWidget, Deferred, Scoped, Source, Tooltip, TypedWidget, Visual, Widget,
};

pub fn hash_of(key: impl Hash) -> u64 {
    let mut hasher = rustc_hash::FxHasher::default();
    key.hash(&mut hasher);
    hasher.finish()
}

pub struct Element {
    pub(crate) widget: Box<dyn AnyWidget>,
    pub(crate) children: Vec<Element>,
    pub(crate) key: Option<u64>,
    pub(crate) branch: Option<u64>,
    pub(crate) anchor: Option<Anchor>,
    pub(crate) style: Option<Box<Style>>,
    pub(crate) visual: Option<Box<Visual>>,
    pub(crate) tooltip: Option<Rc<Tooltip>>,
    pub(crate) scopes: Vec<Rc<Scoped>>,
    pub(crate) deferred: Option<Box<dyn Deferred>>,
    pub(crate) source: Option<Box<dyn Source>>,
}

impl Element {
    pub fn new(widget: impl Widget) -> Self {
        Self::with_children(widget, Vec::new())
    }

    pub fn with_children(widget: impl Widget, mut children: Vec<Element>) -> Self {
        children.retain(|child| !child.is_empty());
        Self::from_parts(Box::new(TypedWidget::new(widget)), children)
    }

    fn from_parts(widget: Box<dyn AnyWidget>, children: Vec<Element>) -> Self {
        crate::stats::element_made();
        Self {
            widget,
            children,
            key: None,
            branch: None,
            anchor: None,
            style: None,
            visual: None,
            tooltip: None,
            scopes: Vec::new(),
            deferred: None,
            source: None,
        }
    }

    pub fn empty() -> Self {
        Self::new(Empty)
    }

    pub fn is_empty(&self) -> bool {
        self.deferred.is_none()
            && self.source.is_none()
            && self.widget.widget_type() == TypeId::of::<Empty>()
    }

    pub fn retained(key: impl Hash) -> Self {
        Self::new(Kept).keyed(key)
    }

    pub(crate) fn is_kept(&self) -> bool {
        self.widget.widget_type() == TypeId::of::<Kept>()
    }

    pub(crate) fn deferring(deferred: Box<dyn Deferred>) -> Self {
        let mut element = Self::new(Empty);
        element.deferred = Some(deferred);
        element
    }

    pub(crate) fn sourced(source: Box<dyn Source>) -> Self {
        let mut element = Self::new(Empty);
        element.source = Some(source);
        element
    }

    #[must_use]
    pub fn keyed(mut self, key: impl Hash) -> Self {
        self.key = Some(hash_of(key));
        self
    }

    #[must_use]
    pub fn key(&self) -> Option<u64> {
        self.key
    }

    #[must_use]
    pub fn anchored(mut self, anchor: Anchor) -> Self {
        self.anchor = Some(anchor);
        self
    }

    #[must_use]
    pub fn anchor(&self) -> Option<Anchor> {
        self.anchor
    }

    #[doc(hidden)]
    #[must_use]
    pub fn branch(mut self, branch: u64) -> Self {
        self.branch = Some(match self.branch {
            Some(inner) => hash_of((branch, inner)),
            None => branch,
        });
        self
    }

    pub(crate) fn scoped(mut self, scoped: Rc<Scoped>) -> Self {
        self.scopes.insert(0, scoped);
        self
    }

    pub(crate) fn visual_mut(&mut self) -> &mut Visual {
        self.visual.get_or_insert_default()
    }

    pub(crate) fn identity(&self) -> TypeId {
        if let Some(source) = &self.source {
            return source.identity();
        }
        match &self.deferred {
            Some(deferred) => deferred.identity(),
            None => self.widget.widget_type(),
        }
    }
}

impl Styled for Element {
    fn style_mut(&mut self) -> &mut Style {
        self.style.get_or_insert_default()
    }
}

struct Empty;

impl Widget for Empty {
    type State = ();

    fn debug_name(&self) -> &'static str {
        "Empty"
    }
}

struct Kept;

impl Widget for Kept {
    type State = ();

    fn debug_name(&self) -> &'static str {
        "Kept"
    }
}

impl std::fmt::Debug for Element {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut node = f.debug_struct(self.widget.debug_name());
        if let Some(key) = self.key {
            node.field("key", &key);
        }
        if !self.children.is_empty() {
            node.field("children", &self.children);
        }
        node.finish()
    }
}
