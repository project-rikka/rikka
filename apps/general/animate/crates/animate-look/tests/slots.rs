use animate_look::{LookValue, Name, Slots};

#[test]
fn slot_edits_undo() {
    let miku = Name::new("miku");
    let rim = Name::new("rim");
    let mut slots = Slots::default();
    let mut undo = vec![slots.assign(miku, 0, Some("skin".into()))];
    undo.push(slots.assign(miku, 1, Some("skin".into())));
    undo.push(slots.set(miku, 0, rim, LookValue::Float(0.4)));
    let assigned = slots.clone();

    let renamed = slots.rename_material("skin", "face");
    assert!(
        slots
            .iter()
            .all(|(_, _, slot)| slot.material.as_deref() == Some("face"))
    );
    slots.apply(renamed);
    assert_eq!(slots, assigned);

    let removed = slots.remove_material("skin");
    assert_eq!(
        slots.slot(miku, 1),
        None,
        "a slot left with nothing is dropped"
    );
    assert_eq!(
        slots.slot(miku, 0).map(|slot| slot.material.clone()),
        Some(None)
    );
    slots.apply(removed);
    assert_eq!(slots, assigned);

    let reset = slots.reset(miku, 0, rim);
    assert!(
        slots
            .slot(miku, 0)
            .is_some_and(|slot| slot.values.is_empty())
    );
    slots.apply(reset);
    let cleared = slots.clear(miku, 0);
    assert_eq!(slots.slot(miku, 0), None);
    slots.apply(cleared);
    assert_eq!(slots, assigned);

    for edit in undo.into_iter().rev() {
        slots.apply(edit);
    }
    assert!(slots.is_empty());
}
