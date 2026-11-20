use std::collections::BTreeMap;

use crate::name::Name;
use crate::value::LookValue;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Slot {
    pub material: Option<String>,
    pub values: BTreeMap<Name, LookValue>,
}

impl Slot {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.material.is_none() && self.values.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum SlotEdit {
    Replace {
        figure: Name,
        material: u32,
        slot: Option<Slot>,
    },
    Batch(Vec<SlotEdit>),
}

impl SlotEdit {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        matches!(self, Self::Batch(edits) if edits.iter().all(Self::is_empty))
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Slots {
    figures: BTreeMap<Name, BTreeMap<u32, Slot>>,
}

impl Slots {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.figures.is_empty()
    }

    #[must_use]
    pub fn slot(&self, figure: Name, material: u32) -> Option<&Slot> {
        self.figures.get(&figure)?.get(&material)
    }

    pub fn figure(&self, figure: Name) -> impl Iterator<Item = (u32, &Slot)> {
        self.figures
            .get(&figure)
            .into_iter()
            .flat_map(|slots| slots.iter().map(|(index, slot)| (*index, slot)))
    }

    pub fn iter(&self) -> impl Iterator<Item = (Name, u32, &Slot)> {
        self.figures.iter().flat_map(|(figure, slots)| {
            slots
                .iter()
                .map(move |(index, slot)| (*figure, *index, slot))
        })
    }

    pub fn apply(&mut self, edit: SlotEdit) -> SlotEdit {
        match edit {
            SlotEdit::Replace {
                figure,
                material,
                slot,
            } => SlotEdit::Replace {
                figure,
                material,
                slot: self.replace(figure, material, slot),
            },
            SlotEdit::Batch(edits) => {
                let mut inverse: Vec<SlotEdit> =
                    edits.into_iter().map(|edit| self.apply(edit)).collect();
                inverse.reverse();
                SlotEdit::Batch(inverse)
            }
        }
    }

    pub fn assign(&mut self, figure: Name, material: u32, key: Option<String>) -> SlotEdit {
        self.change(figure, material, |slot| slot.material = key)
    }

    pub fn clear(&mut self, figure: Name, material: u32) -> SlotEdit {
        self.apply(SlotEdit::Replace {
            figure,
            material,
            slot: None,
        })
    }

    pub fn set(
        &mut self,
        figure: Name,
        material: u32,
        parameter: Name,
        value: LookValue,
    ) -> SlotEdit {
        self.change(figure, material, |slot| {
            slot.values.insert(parameter, value);
        })
    }

    pub fn reset(&mut self, figure: Name, material: u32, parameter: Name) -> SlotEdit {
        self.change(figure, material, |slot| {
            slot.values.remove(&parameter);
        })
    }

    pub fn rename_material(&mut self, from: &str, to: &str) -> SlotEdit {
        self.retarget(from, Some(to))
    }

    pub fn remove_material(&mut self, key: &str) -> SlotEdit {
        self.retarget(key, None)
    }

    fn retarget(&mut self, from: &str, to: Option<&str>) -> SlotEdit {
        let using: Vec<(Name, u32)> = self
            .iter()
            .filter(|(_, _, slot)| slot.material.as_deref() == Some(from))
            .map(|(figure, index, _)| (figure, index))
            .collect();
        let inverse: Vec<SlotEdit> = using
            .into_iter()
            .map(|(figure, index)| {
                self.change(figure, index, |slot| slot.material = to.map(str::to_owned))
            })
            .collect();
        SlotEdit::Batch(inverse)
    }

    fn change(&mut self, figure: Name, material: u32, change: impl FnOnce(&mut Slot)) -> SlotEdit {
        let mut slot = self.slot(figure, material).cloned().unwrap_or_default();
        change(&mut slot);
        self.apply(SlotEdit::Replace {
            figure,
            material,
            slot: Some(slot),
        })
    }

    fn replace(&mut self, figure: Name, material: u32, slot: Option<Slot>) -> Option<Slot> {
        let slot = slot.filter(|slot| !slot.is_empty());
        let slots = self.figures.entry(figure).or_default();
        let previous = match slot {
            Some(slot) => slots.insert(material, slot),
            None => slots.remove(&material),
        };
        if slots.is_empty() {
            self.figures.remove(&figure);
        }
        previous
    }
}
