use std::cmp::Reverse;

#[derive(Debug)]
pub(crate) struct Ring<T> {
    slots: Vec<Slot<T>>,
    frame: u64,
}

#[derive(Debug)]
struct Slot<T> {
    item: T,
    drawn: Option<u64>,
}

impl<T> Ring<T> {
    pub(crate) fn new(items: Vec<T>) -> Self {
        Self {
            slots: items
                .into_iter()
                .map(|item| Slot { item, drawn: None })
                .collect(),
            frame: 0,
        }
    }

    pub(crate) fn acquire(&mut self, busy: impl Fn(&T) -> bool) -> Option<(&T, u32)> {
        let index = self
            .slots
            .iter()
            .enumerate()
            .filter(|(_, slot)| !busy(&slot.item))
            .max_by_key(|(index, slot)| (slot.drawn.map_or(0, |drawn| drawn + 1), Reverse(*index)))
            .map(|(index, _)| index)?;
        self.frame += 1;
        let slot = &mut self.slots[index];
        let age = slot
            .drawn
            .map_or(0, |drawn| u32::try_from(self.frame - drawn).unwrap_or(0));
        slot.drawn = Some(self.frame);
        Some((&slot.item, age))
    }

    pub(crate) fn forget_ages(&mut self) {
        for slot in &mut self.slots {
            slot.drawn = None;
        }
    }

    pub(crate) fn items(&self) -> impl Iterator<Item = &T> {
        self.slots.iter().map(|slot| &slot.item)
    }

    pub(crate) fn items_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.slots.iter_mut().map(|slot| &mut slot.item)
    }
}
