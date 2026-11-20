use rikka_ui::{Align, Element, Space, Styled, View};

use crate::{CaptionBar, container, row_of, spacer};

enum Middle {
    Fill(Element),
    Center(Element),
}

#[must_use]
pub struct Caption {
    leading: Option<Element>,
    middle: Option<Middle>,
    trailing: Option<Element>,
}

pub fn caption() -> Caption {
    Caption {
        leading: None,
        middle: None,
        trailing: None,
    }
}

impl Caption {
    pub fn leading(mut self, content: impl View) -> Self {
        self.leading = Some(content.build());
        self
    }

    pub fn fill(mut self, content: impl View) -> Self {
        self.middle = Some(Middle::Fill(content.build()));
        self
    }

    pub fn center(mut self, content: impl View) -> Self {
        self.middle = Some(Middle::Center(content.build()));
        self
    }

    pub fn trailing(mut self, content: impl View) -> Self {
        self.trailing = Some(content.build());
        self
    }
}

impl std::fmt::Debug for Caption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let middle = match &self.middle {
            Some(Middle::Fill(content)) => Some(("fill", content)),
            Some(Middle::Center(content)) => Some(("center", content)),
            None => None,
        };
        f.debug_struct("Caption")
            .field("leading", &self.leading)
            .field("middle", &middle)
            .field("trailing", &self.trailing)
            .finish_non_exhaustive()
    }
}

impl View for Caption {
    fn build(self) -> Element {
        let middle = match self.middle {
            Some(Middle::Center(center)) => {
                vec![
                    spacer().grow(1.0).build(),
                    center,
                    spacer().grow(1.0).build(),
                ]
            }
            Some(Middle::Fill(fill)) => vec![container(fill).grow(1.0).build()],
            None => vec![spacer().grow(1.0).build()],
        };
        row_of(self.leading.into_iter().chain(middle).chain(self.trailing))
            .gap(Space::SNUG)
            .align_items(Align::Center)
            .build()
    }
}

impl From<Caption> for CaptionBar {
    fn from(caption: Caption) -> Self {
        Self::from(caption.build())
    }
}
