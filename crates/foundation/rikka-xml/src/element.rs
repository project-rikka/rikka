use std::borrow::Cow;
use std::ops::Range;

use crate::namespaces::Declaration;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Element<'a> {
    pub(crate) qualified: &'a str,
    pub(crate) name: &'a str,
    pub(crate) namespace: Option<Cow<'a, str>>,
    pub(crate) attributes: Vec<Attribute<'a>>,
    pub(crate) declarations: Vec<Declaration<'a>>,
    pub(crate) children: Vec<Node<'a>>,
    pub(crate) range: Range<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Attribute<'a> {
    pub(crate) name: &'a str,
    pub(crate) namespace: Option<Cow<'a, str>>,
    pub(crate) value: Cow<'a, str>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Node<'a> {
    Element(Element<'a>),
    Text(Cow<'a, str>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Content<'e, 'a> {
    Element(&'e Element<'a>),
    Text(&'e str),
}

impl<'a> Element<'a> {
    #[must_use]
    pub fn name(&self) -> &'a str {
        self.name
    }

    #[must_use]
    pub fn namespace(&self) -> Option<&str> {
        self.namespace.as_deref()
    }

    #[must_use]
    pub fn range(&self) -> Range<usize> {
        self.range.clone()
    }

    #[must_use]
    pub fn attribute(&self, name: &str) -> Option<&str> {
        self.find_attribute(None, name)
    }

    #[must_use]
    pub fn attribute_in(&self, namespace: &str, name: &str) -> Option<&str> {
        self.find_attribute(Some(namespace), name)
    }

    pub fn attributes(&self) -> impl Iterator<Item = (Option<&str>, &'a str, &str)> {
        self.attributes.iter().map(|attribute| {
            (
                attribute.namespace.as_deref(),
                attribute.name,
                attribute.value.as_ref(),
            )
        })
    }

    pub fn namespaces(&self) -> impl Iterator<Item = (Option<&'a str>, &str)> {
        self.declarations
            .iter()
            .map(|declaration| (declaration.prefix, declaration.uri.as_ref()))
    }

    pub fn children(&self) -> impl Iterator<Item = &Element<'a>> {
        self.children.iter().filter_map(|node| match node {
            Node::Element(element) => Some(element),
            Node::Text(_) => None,
        })
    }

    pub fn contents(&self) -> impl Iterator<Item = Content<'_, 'a>> {
        self.children.iter().map(|node| match node {
            Node::Element(element) => Content::Element(element),
            Node::Text(text) => Content::Text(text),
        })
    }

    pub fn children_named<'e>(&'e self, name: &'e str) -> impl Iterator<Item = &'e Element<'a>> {
        self.children().filter(move |element| element.name == name)
    }

    pub fn descendants(&self) -> impl Iterator<Item = &Element<'a>> {
        Descendants {
            pending: vec![self],
        }
    }

    #[must_use]
    pub fn text(&self) -> Cow<'_, str> {
        let mut fragments = self.children.iter().filter_map(|node| match node {
            Node::Text(text) => Some(text.as_ref()),
            Node::Element(_) => None,
        });
        let Some(first) = fragments.next() else {
            return Cow::Borrowed("");
        };
        let Some(second) = fragments.next() else {
            return Cow::Borrowed(first.trim());
        };
        let mut joined = String::from(first);
        joined.push_str(second);
        joined.extend(fragments);
        Cow::Owned(joined.trim().to_owned())
    }

    fn find_attribute(&self, namespace: Option<&str>, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|attribute| attribute.name == name && attribute.namespace.as_deref() == namespace)
            .map(|attribute| attribute.value.as_ref())
    }
}

struct Descendants<'e, 'a> {
    pending: Vec<&'e Element<'a>>,
}

impl<'e, 'a> Iterator for Descendants<'e, 'a> {
    type Item = &'e Element<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let element = self.pending.pop()?;
        let before = self.pending.len();
        self.pending.extend(element.children());
        self.pending[before..].reverse();
        Some(element)
    }
}
