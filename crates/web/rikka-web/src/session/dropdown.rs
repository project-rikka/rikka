use rikka_gecko_protocol::rikka::dropdown::{
    ChosenWith, DropdownChoice, DropdownId, DropdownItem, ShowDropdown,
};
use rikka_types::Rect;

use crate::wire::Answer;

#[derive(Debug)]
pub struct Dropdown {
    pub id: DropdownId,
    pub anchor: Rect,
    pub items: Vec<DropdownItem>,
    pub selected: Option<u32>,
    answer: Answer,
}

impl Dropdown {
    pub(crate) fn from_wire(shown: ShowDropdown, answer: Answer) -> Dropdown {
        let rect = shown.rect;
        Dropdown {
            id: shown.id,
            anchor: Rect::new(
                rect.left as f32,
                rect.top as f32,
                (rect.right - rect.left) as f32,
                (rect.bottom - rect.top) as f32,
            ),
            items: shown.items,
            selected: shown.selected,
            answer,
        }
    }

    pub fn choose(self, index: u32, with: ChosenWith) {
        self.answer.with(&Some(DropdownChoice { index, with }));
    }

    pub fn dismiss(self) {
        self.answer.with(&None::<DropdownChoice>);
    }
}
