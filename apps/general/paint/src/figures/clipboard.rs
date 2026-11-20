use std::cell::RefCell;

use rikka_app::prelude::Point;
use rikka_figure::Form;

const STEP: f32 = 10.0;

thread_local! {
    static COPIED: RefCell<Option<CopiedFigures>> = const { RefCell::new(None) };
}

struct CopiedFigures {
    forms: Vec<Form>,
    pasted: u32,
}

pub(crate) fn copy(forms: Vec<Form>) {
    if forms.is_empty() {
        return;
    }
    COPIED.with(|cell| *cell.borrow_mut() = Some(CopiedFigures { forms, pasted: 0 }));
}

pub(crate) fn holds() -> bool {
    COPIED.with(|cell| cell.borrow().is_some())
}

pub(crate) fn paste() -> Vec<Form> {
    COPIED.with(|cell| {
        let mut contents = cell.borrow_mut();
        let Some(copied) = contents.as_mut() else {
            return Vec::new();
        };
        copied.pasted += 1;
        let offset = STEP * copied.pasted as f32;
        copied
            .forms
            .iter()
            .map(|form| form.moved(Point::new(offset, offset)))
            .collect()
    })
}

pub(crate) fn offset(forms: &[Form]) -> Vec<Form> {
    forms
        .iter()
        .map(|form| form.moved(Point::new(STEP, STEP)))
        .collect()
}
