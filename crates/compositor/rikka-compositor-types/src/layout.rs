use std::cell::RefCell;
use std::rc::Rc;

use rikka_compositor_geometry::{Logical, Rect, Transform};
use slotmap::SecondaryMap;

use crate::output_id::OutputId;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OutputLayout {
    pub geometry: Rect<i32, Logical>,
    pub scale: f64,
    pub transform: Transform,
}

type Table = Rc<RefCell<SecondaryMap<OutputId, OutputLayout>>>;

#[derive(Debug, Default)]
pub struct Layouts(Table);

#[derive(Debug, Clone)]
pub struct LayoutView(Table);

impl Layouts {
    pub fn set(&mut self, output: OutputId, layout: OutputLayout) {
        self.0.borrow_mut().insert(output, layout);
    }

    pub fn remove(&mut self, output: OutputId) {
        self.0.borrow_mut().remove(output);
    }

    pub fn view(&self) -> LayoutView {
        LayoutView(Rc::clone(&self.0))
    }
}

impl LayoutView {
    pub fn get(&self, output: OutputId) -> Option<OutputLayout> {
        self.0.borrow().get(output).copied()
    }
}
