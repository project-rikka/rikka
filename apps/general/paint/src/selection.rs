use std::cell::RefCell;
use std::rc::Rc;

use paint_document::{Area, Combine, Edge, Region};
use rikka_ui::{Observed, Observers};

#[derive(Clone, Debug, Default)]
pub(crate) struct Selection(Rc<Inner>);

#[derive(Debug, Default)]
struct Inner {
    committed: RefCell<Rc<Region>>,
    display: RefCell<DisplayedSelection>,
    observers: Observers,
}

#[derive(Debug, Default)]
struct DisplayedSelection {
    region: Rc<Region>,
    outline: Rc<[Edge]>,
    offset: (i32, i32),
}

impl Selection {
    pub(crate) fn same(&self, other: &Selection) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }

    pub(crate) fn region(&self) -> Rc<Region> {
        self.0.observers.read();
        Rc::clone(&self.0.committed.borrow())
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.0.observers.read();
        self.0.committed.borrow().is_empty()
    }

    pub(crate) fn shown_bounds(&self) -> Option<Area> {
        self.0.observers.read();
        self.0.display.borrow().region.bounds()
    }

    pub(crate) fn outline(&self) -> Rc<[Edge]> {
        self.0.observers.read();
        Rc::clone(&self.0.display.borrow().outline)
    }

    pub(crate) fn offset(&self) -> (i32, i32) {
        self.0.observers.read();
        self.0.display.borrow().offset
    }

    pub(crate) fn holds(&self, x: f32, y: f32) -> bool {
        x >= 0.0 && y >= 0.0 && self.0.committed.borrow().coverage(x as u32, y as u32) > u8::MAX / 2
    }

    pub(crate) fn displace(&self, offset: (i32, i32)) {
        let mut display = self.0.display.borrow_mut();
        if display.offset != offset {
            display.offset = offset;
            drop(display);
            self.0.observers.changed();
        }
    }

    pub(crate) fn preview(&self, combine: Combine, region: Region) {
        let combined = self.0.committed.borrow().combine(combine, region);
        if self.show(Rc::new(combined)) {
            self.0.observers.changed();
        }
    }

    pub(crate) fn commit(&self) {
        let displayed = Rc::clone(&self.0.display.borrow().region);
        let previous = self.0.committed.replace(Rc::clone(&displayed));
        if !Rc::ptr_eq(&previous, &displayed) {
            self.0.observers.changed();
        }
    }

    pub(crate) fn withdraw(&self) {
        let committed = Rc::clone(&self.0.committed.borrow());
        if self.show(committed) {
            self.0.observers.changed();
        }
    }

    pub(crate) fn adopt(&self, region: Region) {
        if *self.0.committed.borrow().as_ref() != region {
            self.set(region);
        }
        self.displace((0, 0));
    }

    pub(crate) fn set(&self, region: Region) {
        let region = Rc::new(region);
        *self.0.committed.borrow_mut() = Rc::clone(&region);
        self.show(region);
        self.0.observers.changed();
    }

    fn show(&self, region: Rc<Region>) -> bool {
        let mut display = self.0.display.borrow_mut();
        if *display.region == *region && display.offset == (0, 0) {
            display.region = region;
            return false;
        }
        if *display.region != *region {
            display.outline = Rc::from(region.outline());
        }
        display.region = region;
        display.offset = (0, 0);
        true
    }
}

impl Observed for Selection {
    fn observers(&self) -> &Observers {
        &self.0.observers
    }
}
