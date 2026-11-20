use std::collections::BTreeSet;

use animate_ui::glyph;
use rikka_app::prelude::*;

use crate::actions::{Starter, users};
use crate::state::Popup;
use crate::traits::{Desk, SlotFigure};
use crate::views::wording;

const WIDTH: f32 = 260.0;
const MENU_WIDTH: f32 = 220.0;
const ASSIGN: &str = "animate-shader-assign";
const CLASSES: &str = "animate-shader-classes";

pub(crate) fn slots(desk: Desk<'_>, figure: SlotFigure) -> Element {
    let chosen: Vec<u32> = desk.shader().chosen_slots(figure.name).to_vec();
    let look = desk.look();
    let name = figure.name;
    let rows = figure.materials.iter().map(|material| {
        let assigned = look
            .slots
            .slot(name, material.index)
            .and_then(|slot| slot.material.clone());
        let held = match &assigned {
            Some(key) => Label::from(key.clone()),
            None => tr!("animate.shader.imported"),
        };
        let item = list_item(Label::from(material.name.clone()))
            .trailing(held)
            .check(chosen.contains(&material.index))
            .keyed(("slot", material.index));
        let index = material.index;
        let item = item.on_check(desk.act_with(move |studio, _: bool| {
            studio.choose_slot(name, index, true);
        }));
        match material.class {
            Some(class) => item.secondary(wording::class(class)),
            None => item,
        }
    });
    let indices: Vec<u32> = figure
        .materials
        .iter()
        .map(|material| material.index)
        .collect();
    let selected = chosen
        .first()
        .and_then(|first| indices.iter().position(|index| index == first));
    let list = list(rows)
        .selected(selected)
        .on_select(desk.act_with(move |studio, at: usize| {
            if let Some(index) = indices.get(at) {
                studio.choose_slot(name, *index, false);
            }
        }))
        .fill_width();
    column_of([
        header(desk, &figure, &chosen),
        actions(desk, &chosen),
        rule().any(),
        scrollable(list).grow(1.0).fill_width().any(),
    ])
    .gap(Space::TIGHT)
    .padding(Inset::all(8.0))
    .width(WIDTH)
    .fill_height()
    .any()
}

fn header(desk: Desk<'_>, figure: &SlotFigure, chosen: &[u32]) -> Element {
    let name = figure.name;
    let every: Vec<u32> = figure
        .materials
        .iter()
        .map(|material| material.index)
        .collect();
    let all = every.len() == chosen.len() && !every.is_empty();
    let mut parts = vec![
        text(Label::from(figure.name.to_string()))
            .bold()
            .no_wrap()
            .grow(1.0)
            .any(),
        glyph(MaterialSymbol::SELECT_ALL)
            .ghost()
            .on_press(desk.act(move |studio| {
                let picked = if all { Vec::new() } else { every.clone() };
                studio.choose_slots(name, picked);
            }))
            .tooltip(tr!("animate.shader.all_slots")),
    ];
    let classes: BTreeSet<&'static str> = figure
        .materials
        .iter()
        .filter_map(|material| material.class)
        .collect();
    if !classes.is_empty() {
        parts.push(
            glyph(MaterialSymbol::CATEGORY)
                .ghost()
                .on_press(desk.act(|studio| studio.toggle_popup(Popup::Classes)))
                .tooltip(tr!("animate.shader.slots_by_class"))
                .build()
                .keyed(CLASSES),
        );
        if desk.shader().popup == Some(Popup::Classes) {
            let items = classes.into_iter().map(|class| {
                let picked: Vec<u32> = figure
                    .materials
                    .iter()
                    .filter(|material| material.class == Some(class))
                    .map(|material| material.index)
                    .collect();
                menu_item(wording::class(class))
                    .on_press(desk.act(move |studio| studio.choose_slots(name, picked.clone())))
            });
            parts.push(
                menu()
                    .items(items)
                    .width(MENU_WIDTH)
                    .on_dismiss(desk.act(|studio| studio.close_popup()))
                    .build()
                    .keyed((CLASSES, "menu"))
                    .anchored(Anchor::below(CLASSES)),
            );
        }
    }
    row_of(parts)
        .gap(Space::TIGHT)
        .align_items(Align::Center)
        .fill_width()
        .any()
}

fn actions(desk: Desk<'_>, chosen: &[u32]) -> Element {
    let any = !chosen.is_empty();
    let mut parts = vec![
        button(text(tr!("animate.shader.assign")).caption())
            .small()
            .focusless()
            .enabled(any)
            .on_press(desk.act(|studio| studio.toggle_popup(Popup::Assign)))
            .build()
            .keyed(ASSIGN),
        glyph(MaterialSymbol::CONTENT_COPY)
            .ghost()
            .enabled(any)
            .on_press(desk.act(|studio| studio.make_slots_unique()))
            .tooltip(tr!("animate.shader.make_unique")),
        glyph(MaterialSymbol::RESTART_ALT)
            .ghost()
            .enabled(any)
            .on_press(desk.act(|studio| studio.reset_slots()))
            .tooltip(tr!("animate.shader.reset_slots")),
    ];
    if any && desk.shader().popup == Some(Popup::Assign) {
        parts.push(assign_menu(desk));
    }
    row_of(parts)
        .gap(Space::TIGHT)
        .align_items(Align::Center)
        .fill_width()
        .any()
}

fn assign_menu(desk: Desk<'_>) -> Element {
    let look = desk.look();
    let mut existing = Vec::new();
    for key in look.materials.keys() {
        let assigned = key.clone();
        existing.push(
            menu_item(tr!(
                "animate.shader.assigned_material",
                name = key.as_str(),
                count = users(look, key)
            ))
            .on_press(desk.act(move |studio| studio.assign_slots(Some(assigned.clone())))),
        );
    }
    let mut starters = vec![
        menu_item(tr!("animate.shader.empty_material"))
            .on_press(desk.act(|studio| studio.assign_new(Starter::Empty))),
    ];
    for preset in rikka_shader_presets::presets() {
        starters.push(
            menu_item(Label::from(preset.title))
                .on_press(desk.act(move |studio| studio.assign_new(Starter::Preset(preset)))),
        );
    }
    let mut items = vec![
        menu_item(tr!("animate.shader.new_material"))
            .submenu(true)
            .items(starters),
    ];
    if !existing.is_empty() {
        items.push(menu_separator());
        items.extend(existing);
    }
    items.push(menu_separator());
    items.push(
        menu_item(tr!("animate.shader.imported"))
            .on_press(desk.act(|studio| studio.assign_slots(None))),
    );
    menu()
        .items(items)
        .width(MENU_WIDTH)
        .on_dismiss(desk.act(|studio| studio.close_popup()))
        .build()
        .keyed((ASSIGN, "menu"))
        .anchored(Anchor::below(ASSIGN))
}
