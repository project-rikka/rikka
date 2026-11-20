use std::borrow::Cow;

const XML_PREFIX: &str = "xml";
const XML_NAMESPACE: &str = "http://www.w3.org/XML/1998/namespace";
const DECLARATION: &str = "xmlns";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Declaration<'a> {
    pub(crate) prefix: Option<&'a str>,
    pub(crate) uri: Cow<'a, str>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Role<'a> {
    Declares(Option<&'a str>),
    Attribute,
}

impl<'a> Role<'a> {
    pub(crate) fn of(name: &'a str) -> Self {
        match name.split_once(':') {
            None if name == DECLARATION => Self::Declares(None),
            Some((DECLARATION, prefix)) => Self::Declares(Some(prefix)),
            _ => Self::Attribute,
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct Scopes<'a> {
    declared: Vec<Declaration<'a>>,
}

impl<'a> Scopes<'a> {
    pub(crate) fn depth(&self) -> usize {
        self.declared.len()
    }

    pub(crate) fn declare(&mut self, declarations: &[Declaration<'a>]) {
        self.declared.extend_from_slice(declarations);
    }

    pub(crate) fn leave(&mut self, depth: usize) {
        self.declared.truncate(depth);
    }

    pub(crate) fn resolve(&self, prefix: Option<&str>) -> Resolved<'a> {
        if prefix == Some(XML_PREFIX) {
            return Resolved::Namespace(Cow::Borrowed(XML_NAMESPACE));
        }
        let found = self
            .declared
            .iter()
            .rev()
            .find(|declaration| declaration.prefix == prefix);
        match (found, prefix) {
            (Some(declaration), _) if !declaration.uri.is_empty() => {
                Resolved::Namespace(declaration.uri.clone())
            }
            (None, Some(_)) => Resolved::Undeclared,
            _ => Resolved::None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Resolved<'a> {
    Namespace(Cow<'a, str>),
    None,
    Undeclared,
}
