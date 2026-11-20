use rikka_llm::Effort;
use rikka_llm_accounts::{Llm, ModelOption, ModelOptions, Row, Selection, effort_label};

use crate::chat::Chat;
use crate::prelude::*;

const TRIGGER: &str = "chat-model";
const PANEL: &str = "chat-model-panel";

const LEVELS: f32 = 152.0;
const GAP: f32 = 4.0;
const OVERLAP: f32 = -2.0;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Picking {
    open: bool,
    reasoning: Option<f32>,
}

impl Picking {
    #[must_use]
    pub fn new(open: bool) -> Picking {
        Picking {
            open,
            reasoning: None,
        }
    }

    #[must_use]
    pub fn is_open(self) -> bool {
        self.open
    }

    #[must_use]
    pub fn with_reasoning(self, at: Option<f32>) -> Picking {
        Picking {
            reasoning: at,
            ..self
        }
    }
}

#[derive(Debug, Default)]
pub struct Models(ModelOptions);

impl Models {
    #[must_use]
    pub fn new(llm: &Llm, selection: Option<&Selection>) -> Models {
        Models(ModelOptions::new(llm, selection).with_effort_row())
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn trigger<C: Chat>(&self, picking: Picking, width: f32) -> Element {
        let active = self.active_selections();
        let chose = C::chose_model();
        dropdown(self.0.selected_index())
            .options(self.0.options().iter().map(named_pick))
            .placeholder(C::pick_model())
            .open(picking.open)
            .enabled(!self.is_empty())
            .on_toggle(C::pick_models())
            .on_select(Handler::relaying(
                move |reach, (at,): (usize,)| match active.get(at) {
                    Some(selection) => chose.call(reach, (selection.clone(),)),
                    None => {
                        skip_view();
                        Some(())
                    }
                },
            ))
            .control_size(ControlSize::Small)
            .width(width)
            .build()
            .keyed(TRIGGER)
    }

    pub fn panels<C: Chat>(&self, picking: Picking) -> Vec<Element> {
        if !picking.open || self.is_empty() {
            return Vec::new();
        }
        let mut panels = vec![self.list::<C>()];
        if let Some(at) = picking.reasoning.filter(|_| self.0.has_levels()) {
            panels.push(self.levels::<C>(at));
        }
        panels
    }

    fn list<C: Chat>(&self) -> Element {
        let items: Vec<MenuItem> = self
            .0
            .rows()
            .iter()
            .map(|row| match row {
                Row::Account(at) => menu_header(self.0.options()[*at].account()),
                Row::Model(at) => menu_item(self.0.options()[*at].name())
                    .selected(self.0.selected_index() == Some(*at)),
                Row::Separator => menu_separator(),
                Row::Levels => menu_item(C::reasoning()).submenu(true),
            })
            .collect();
        let picks = self.rows();
        let chose = C::chose_model();
        let unfold = C::unfold_reasoning();
        let folded = C::unfold_reasoning();

        menu()
            .items(items)
            .control_size(ControlSize::Small)
            .on_activate(Handler::relaying(
                move |reach, (row,): (usize,)| match picks.get(row).cloned().flatten() {
                    Some(selection) => chose.call(reach, (selection,)),
                    None => {
                        skip_view();
                        Some(())
                    }
                },
            ))
            .on_highlight(folded.map(|_: (Option<usize>,)| (None,)))
            .on_open(unfold.map(|(_, row): (usize, Rect)| (Some(row.y()),)))
            .on_dismiss(C::pick_models().map(|()| (false,)))
            .build()
            .keyed(PANEL)
            .anchored(Anchor::above(TRIGGER).offset(0.0, GAP).match_target_width())
    }

    fn levels<C: Chat>(&self, at: f32) -> Element {
        let efforts: Vec<Effort> = self.0.efforts().to_vec();
        let selected = self.0.selected().and_then(ModelOption::nearest_effort);
        let items: Vec<MenuItem> = efforts
            .iter()
            .map(|effort| menu_item(effort_label(*effort)).selected(Some(*effort) == selected))
            .collect();
        let chose = C::chose_effort();

        menu()
            .items(items)
            .control_size(ControlSize::Small)
            .width(LEVELS)
            .on_activate(Handler::relaying(
                move |reach, (row,): (usize,)| match efforts.get(row) {
                    Some(effort) => chose.call(reach, (*effort,)),
                    None => {
                        skip_view();
                        Some(())
                    }
                },
            ))
            .on_dismiss(C::unfold_reasoning().map(|()| (None,)))
            .build()
            .anchored(Anchor::beside(PANEL).offset(OVERLAP, at))
    }

    fn active_selections(&self) -> Vec<Selection> {
        (0..self.0.options().len())
            .filter_map(|at| self.0.selection_at(at))
            .collect()
    }

    fn rows(&self) -> Vec<Option<Selection>> {
        (0..self.0.rows().len())
            .map(|row| self.0.selection_for_row(row))
            .collect()
    }
}

fn named_pick(pick: &ModelOption) -> Label {
    named(pick.name(), pick.nearest_effort())
}

pub(crate) fn named(name: &str, effort: Option<Effort>) -> Label {
    match effort {
        Some(level) => Label::from(format!("{name} ({})", effort_label(level))),
        None => Label::from(name),
    }
}
