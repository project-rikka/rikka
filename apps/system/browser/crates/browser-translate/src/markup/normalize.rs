use crate::markup::decode::{Arena, Item, Parent};
use crate::markup::plan::{Kind, Piece, Plan};
use crate::unit::{Output, OutputNode};

pub(crate) fn normalize(plan: &Plan, mut arena: Arena) -> Output {
    place(plan, &mut arena);
    restore_text(plan, &mut arena);
    Output {
        nodes: emit(plan, &arena, Parent::Root),
    }
}

fn place(plan: &Plan, arena: &mut Arena) {
    for (slot, planned) in plan.slots.iter().enumerate() {
        let expected = planned.parent.map_or(Parent::Root, Parent::Slot);
        match arena.parents[slot] {
            Some(actual) if actual == expected => continue,
            Some(actual) => {
                arena
                    .items_mut(actual)
                    .retain(|item| *item != Item::Slot(slot));
            }
            None => {
                arena.slots[slot] = Some(Vec::new());
            }
        }
        let items = arena.items_mut(expected);
        let at = planned
            .previous
            .and_then(|previous| items.iter().position(|item| *item == Item::Slot(previous)))
            .map_or(0, |position| position + 1);
        items.insert(at, Item::Slot(slot));
        arena.parents[slot] = Some(expected);
    }
}

fn restore_text(plan: &Plan, arena: &mut Arena) {
    for (slot, planned) in plan.slots.iter().enumerate().rev() {
        if planned.kind != Kind::Element || !planned.has_text {
            continue;
        }
        if has_text(plan, arena, Parent::Slot(slot)) {
            continue;
        }
        arena.slots[slot] = Some(
            planned
                .children
                .iter()
                .map(|piece| match piece {
                    Piece::Text(text) => Item::Text(text.clone()),
                    Piece::Slot(child) => Item::Slot(*child),
                })
                .collect(),
        );
    }
}

fn has_text(plan: &Plan, arena: &Arena, parent: Parent) -> bool {
    arena.items(parent).iter().any(|item| match item {
        Item::Text(text) => !text.trim().is_empty(),
        Item::Slot(slot) => match plan.slots[*slot].kind {
            Kind::Token(_) => true,
            Kind::Keep => false,
            Kind::Element => has_text(plan, arena, Parent::Slot(*slot)),
        },
    })
}

fn emit(plan: &Plan, arena: &Arena, parent: Parent) -> Vec<OutputNode> {
    let mut outs = Vec::new();
    for item in arena.items(parent) {
        match item {
            Item::Text(text) => push_text(&mut outs, text),
            Item::Slot(slot) => {
                let planned = &plan.slots[*slot];
                match &planned.kind {
                    Kind::Token(word) => push_text(&mut outs, word),
                    Kind::Keep => outs.push(OutputNode::Keep(planned.id)),
                    Kind::Element => outs.push(OutputNode::Element {
                        id: planned.id,
                        nodes: emit(plan, arena, Parent::Slot(*slot)),
                    }),
                }
            }
        }
    }
    outs
}

fn push_text(outs: &mut Vec<OutputNode>, text: &str) {
    if text.is_empty() {
        return;
    }
    if let Some(OutputNode::Text(last)) = outs.last_mut() {
        last.push_str(text);
    } else {
        outs.push(OutputNode::Text(text.to_owned()));
    }
}
