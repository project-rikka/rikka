use animate_look::{Look, LookEdit, Name, Slot, SlotEdit};

use crate::actions::structure::Starter;
use crate::traits::{SlotFigure, Workshop};

fn assigning(look: &Look, figure: Name, materials: &[u32], key: Option<&str>) -> SlotEdit {
    SlotEdit::Batch(
        materials
            .iter()
            .map(|material| {
                let mut slot = look
                    .slots
                    .slot(figure, *material)
                    .cloned()
                    .unwrap_or_default();
                slot.material = key.map(str::to_owned);
                SlotEdit::Replace {
                    figure,
                    material: *material,
                    slot: Some(slot),
                }
            })
            .collect(),
    )
}

pub(crate) fn users(look: &Look, key: &str) -> usize {
    look.slots
        .iter()
        .filter(|(_, _, slot)| slot.material.as_deref() == Some(key))
        .count()
}

impl dyn Workshop + '_ {
    pub(crate) fn choose_slot(&mut self, figure: Name, material: u32, extend: bool) {
        let shader = self.shader_mut();
        if shader.slot_figure != Some(figure) {
            shader.slot_figure = Some(figure);
            shader.slots.clear();
        }
        match (
            extend,
            shader.slots.iter().position(|held| *held == material),
        ) {
            (true, Some(at)) => {
                shader.slots.remove(at);
            }
            (true, None) => shader.slots.push(material),
            (false, _) => shader.slots = vec![material],
        }
        self.follow_slots(figure);
    }

    pub(crate) fn choose_slots(&mut self, figure: Name, materials: Vec<u32>) {
        self.close_popup();
        let shader = self.shader_mut();
        shader.slot_figure = Some(figure);
        shader.slots = materials;
        self.follow_slots(figure);
    }

    fn follow_slots(&mut self, figure: Name) {
        let chosen = self.shader().slots.clone();
        self.highlight_slots(figure, &chosen);
        if !chosen.is_empty() {
            self.reveal_properties();
        }
        let shown = match chosen.as_slice() {
            [one] => self
                .look()
                .slots
                .slot(figure, *one)
                .and_then(|slot| slot.material.clone()),
            _ => None,
        };
        if let Some(key) = shown {
            self.show_material(key);
        }
    }

    pub(crate) fn chosen_figure_slots(&self) -> Option<(SlotFigure, Vec<u32>)> {
        let figure = self.slot_figure()?;
        let chosen = self.shader().chosen_slots(figure.name).to_vec();
        Some((figure, chosen))
    }

    pub(crate) fn assign_slots(&mut self, key: Option<String>) {
        self.close_popup();
        let Some((figure, chosen)) = self.chosen_figure_slots() else {
            return;
        };
        let edit = assigning(self.look(), figure.name, &chosen, key.as_deref());
        if self.apply_look(LookEdit::Slots(edit), None)
            && let Some(key) = key
        {
            self.show_material(key);
        }
    }

    pub(crate) fn assign_new(&mut self, starter: Starter) {
        self.close_popup();
        let Some((figure, chosen)) = self.chosen_figure_slots() else {
            return;
        };
        let graph = match starter.graph(self.library()) {
            Ok(graph) => graph,
            Err(error) => return self.fail(error),
        };
        let added = self
            .workbench()
            .add_material(self.look(), starter.key(), graph);
        let (key, insert) = match added {
            Ok(added) => added,
            Err(error) => return self.fail(error),
        };
        let assign = assigning(self.look(), figure.name, &chosen, Some(&key));
        if self.apply_look(LookEdit::Batch(vec![insert, LookEdit::Slots(assign)]), None) {
            self.show_material(key);
        }
    }

    pub(crate) fn make_slots_unique(&mut self) {
        self.close_popup();
        let Some((figure, chosen)) = self.chosen_figure_slots() else {
            return;
        };
        let mut look = self.look().clone();
        let library = std::sync::Arc::clone(self.workbench().library());
        let mut edits = Vec::new();
        let mut last = None;
        for material in chosen {
            let Some(key) = look
                .slots
                .slot(figure.name, material)
                .and_then(|slot| slot.material.clone())
            else {
                continue;
            };
            if users(&look, &key) < 2 {
                continue;
            }
            let copied = self.workbench().duplicate_material(&look, &key);
            let (copy, insert) = match copied {
                Ok(copied) => copied,
                Err(error) => return self.fail(error),
            };
            let assign = LookEdit::Slots(assigning(&look, figure.name, &[material], Some(&copy)));
            for edit in [insert, assign] {
                if let Err(error) = look.apply(&library, edit.clone()) {
                    return self.fail(error);
                }
                edits.push(edit);
            }
            last = Some(copy);
        }
        if edits.is_empty() {
            return;
        }
        if self.apply_look(LookEdit::Batch(edits), None)
            && let Some(key) = last
        {
            self.show_material(key);
        }
    }

    pub(crate) fn reset_slots(&mut self) {
        self.close_popup();
        let Some((figure, chosen)) = self.chosen_figure_slots() else {
            return;
        };
        let edit = SlotEdit::Batch(
            chosen
                .into_iter()
                .map(|material| SlotEdit::Replace {
                    figure: figure.name,
                    material,
                    slot: None::<Slot>,
                })
                .collect(),
        );
        self.apply_look(LookEdit::Slots(edit), None);
    }
}
