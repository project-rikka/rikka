use rikka_polymesh::Polymesh;

use crate::context::Context;
use crate::evaluators::{self, Scope, Slot};
use crate::modifier::{Modifier, Purpose};
use crate::note::{Note, NoteKind, SkipReason};
use crate::origins::{self, Origins};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Evaluated {
    pub mesh: Polymesh,
    pub origins: Origins,
    pub control_edges: Option<Vec<bool>>,
    pub notes: Vec<Note>,
}

#[must_use]
pub fn cage_index(stack: &[Modifier]) -> Option<usize> {
    let mut cage = None;
    for (index, modifier) in stack.iter().enumerate() {
        let kind = modifier.modifier_type();
        if !kind.supports_edit_mode() {
            continue;
        }
        if !modifier.show_viewport || !modifier.show_in_editmode {
            continue;
        }
        if !kind.supports_mapping() {
            break;
        }
        if modifier.show_on_cage {
            cage = Some(index);
        }
    }
    cage
}

#[derive(Clone, Default)]
pub struct StackCache {
    slots: Vec<Slot>,
}

impl StackCache {
    #[must_use]
    pub fn new() -> StackCache {
        StackCache::default()
    }

    pub fn clear(&mut self) {
        self.slots.clear();
    }
}

impl std::fmt::Debug for StackCache {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("StackCache")
            .field("slots", &self.slots.len())
            .finish()
    }
}

#[must_use]
pub fn evaluate(
    cage: &Polymesh,
    stack: &[Modifier],
    ctx: &dyn Context,
    purpose: Purpose,
) -> Evaluated {
    run_stack(cage, stack, ctx, purpose, None)
}

#[must_use]
pub fn evaluate_cached(
    cage: &Polymesh,
    stack: &[Modifier],
    ctx: &dyn Context,
    purpose: Purpose,
    cache: &mut StackCache,
) -> Evaluated {
    cache.slots.resize_with(stack.len(), Slot::default);
    run_stack(cage, stack, ctx, purpose, Some(&mut cache.slots))
}

fn run_stack(
    cage: &Polymesh,
    stack: &[Modifier],
    ctx: &dyn Context,
    purpose: Purpose,
    mut cache: Option<&mut [Slot]>,
) -> Evaluated {
    let count = match purpose {
        Purpose::EditCage => cage_index(stack).map_or(0, |index| index + 1),
        _ => stack.len(),
    };
    let mut mesh = cage.clone();
    origins::attach(&mut mesh);
    let mut notes = Vec::new();
    let mut slots = None;
    for (index, modifier) in stack.iter().enumerate().take(count) {
        if !modifier.is_enabled(purpose) {
            let reason = match purpose {
                Purpose::EditMode | Purpose::EditCage
                    if !modifier.modifier_type().supports_edit_mode() =>
                {
                    SkipReason::EditModeUnsupported
                }
                _ => SkipReason::Hidden(purpose),
            };
            notes.push(Note {
                modifier: index,
                kind: NoteKind::Skipped(reason),
            });
            continue;
        }
        let mut scope = Scope::new(ctx, purpose).with_material_slots(slots.clone());
        let slot = cache.as_deref_mut().and_then(|slots| slots.get_mut(index));
        let outcome = evaluators::run(&modifier.kind, &mut mesh, &mut scope, slot);
        for kind in scope.into_notes() {
            if let NoteKind::MaterialSlots(replaced) = &kind {
                slots = Some(replaced.clone());
            }
            notes.push(Note {
                modifier: index,
                kind,
            });
        }
        if let Err(reason) = outcome {
            notes.push(Note {
                modifier: index,
                kind: NoteKind::Skipped(reason),
            });
        }
    }
    let (origins, control_edges) = origins::detach(&mut mesh);
    Evaluated {
        mesh,
        origins,
        control_edges,
        notes,
    }
}
