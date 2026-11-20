use rikka_ui::{Element, View};

#[doc(hidden)]
pub use rikka_ui_macros::children as __children;

#[doc(hidden)]
#[derive(Debug, Default)]
pub struct Children(Vec<Element>);

impl Children {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, view: impl View) {
        let element = view.build();
        if !element.is_empty() {
            self.0.push(element);
        }
    }

    pub fn branded(view: impl View, branch: u64) -> Element {
        view.build().branch(branch)
    }

    pub fn mark(&self) -> usize {
        self.0.len()
    }

    pub fn brand(&mut self, from: usize, branch: u64) {
        for element in &mut self.0[from..] {
            let element_value = std::mem::replace(element, Element::empty());
            *element = element_value.branch(branch);
        }
    }
}

impl IntoIterator for Children {
    type Item = Element;
    type IntoIter = std::vec::IntoIter<Element>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

pub(crate) fn built(children: impl IntoIterator<Item = impl View>) -> Vec<Element> {
    let mut built: Vec<Element> = children.into_iter().map(View::build).collect();
    built.retain(|element| !element.is_empty());
    built
}

#[macro_export]
#[doc(hidden)]
macro_rules! __column {
    ($($body:tt)*) => {
        $crate::column_of($crate::__children!($crate; $($body)*))
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! __row {
    ($($body:tt)*) => {
        $crate::row_of($crate::__children!($crate; $($body)*))
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! __page {
    ($($body:tt)*) => {
        $crate::page($crate::__children!($crate; $($body)*))
    };
}
