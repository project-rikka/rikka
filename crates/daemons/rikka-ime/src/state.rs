use rikka_types::Rect;

use crate::candidate::{Candidate, CandidateLayout, Candidates};
use crate::composing::Composing;
use crate::composition::Preedit;
use crate::names;
use crate::property::Property;
use crate::status_area::{Menu, StatusArea};

#[derive(Debug, Default)]
pub(crate) struct State {
    preedit: String,
    attributes: String,
    caret: usize,
    preedit_shown: bool,
    aux: String,
    aux_shown: bool,
    candidates: Candidates,
    candidates_shown: bool,
    spot: Option<Rect>,
    properties: Vec<Property>,
    input_methods: Vec<Property>,
    menu: Option<Menu>,
    menu_requested: Option<String>,
    enabled: bool,
}

impl State {
    #[must_use]
    pub(crate) fn composing(&self) -> Composing {
        Composing {
            preedit: match self.preedit_shown {
                true => Preedit::marked(&self.preedit, &self.attributes, self.caret),
                false => Preedit::default(),
            },
            aux: match self.aux_shown {
                true => self.aux.clone(),
                false => String::new(),
            },
            candidates: match self.candidates_shown {
                true => self.candidates.clone(),
                false => Candidates::default(),
            },
            spot: self.spot,
            status: self.enabled.then(|| self.current()).flatten().cloned(),
        }
    }

    #[must_use]
    pub(crate) fn status_area(&self) -> StatusArea {
        if !self.enabled {
            return StatusArea::default();
        }
        StatusArea {
            properties: self.properties.clone(),
            input_methods: self.input_methods.clone(),
            menu: self.menu.clone(),
        }
    }

    #[must_use]
    pub(crate) fn shows_status_area(&self, area: &StatusArea) -> bool {
        match self.enabled {
            true => {
                area.properties == self.properties
                    && area.input_methods == self.input_methods
                    && area.menu == self.menu
            }
            false => *area == StatusArea::default(),
        }
    }

    fn current(&self) -> Option<&Property> {
        self.properties
            .iter()
            .find(|property| property.is_current_input_method())
    }

    pub(crate) fn set_preedit(&mut self, text: String, attributes: String) {
        self.preedit = text;
        self.attributes = attributes;
    }

    pub(crate) fn set_caret(&mut self, caret: i32) {
        self.caret = usize::try_from(caret).unwrap_or(0);
    }

    pub(crate) fn show_preedit(&mut self, shown: bool) {
        self.preedit_shown = shown;
    }

    pub(crate) fn set_aux(&mut self, text: String) {
        self.aux = text;
    }

    pub(crate) fn show_aux(&mut self, shown: bool) {
        self.aux_shown = shown;
    }

    pub(crate) fn set_page(
        &mut self,
        labels: Vec<String>,
        texts: Vec<String>,
        previous: bool,
        more: bool,
        cursor: i32,
        layout: i32,
    ) {
        self.set_candidates(labels, texts, previous, more);
        self.candidates.focused = usize::try_from(cursor).ok();
        self.candidates.layout = CandidateLayout::from_hint(layout);
        self.clamp_focus();
    }

    pub(crate) fn set_candidates(
        &mut self,
        labels: Vec<String>,
        texts: Vec<String>,
        previous: bool,
        more: bool,
    ) {
        self.candidates.entries = labels
            .into_iter()
            .zip(texts)
            .map(|(label, text)| Candidate::new(label, text))
            .collect();
        self.candidates.previous = previous;
        self.candidates.more = more;
        self.clamp_focus();
    }

    pub(crate) fn set_focus(&mut self, cursor: i32) {
        self.candidates.focused = usize::try_from(cursor).ok();
        self.clamp_focus();
    }

    pub(crate) fn show_candidates(&mut self, shown: bool) {
        self.candidates_shown = shown;
    }

    pub(crate) fn set_spot(&mut self, spot: Rect) {
        self.spot = Some(spot);
    }

    pub(crate) fn update_property(&mut self, property: &str) {
        let Some(property) = Property::parse(property) else {
            return;
        };
        match self
            .properties
            .iter_mut()
            .find(|entry| entry.key == property.key)
        {
            Some(entry) => *entry = property,
            None => self.properties.push(property),
        }
    }

    pub(crate) fn register_properties(&mut self, properties: &[String]) {
        self.properties = parsed(properties);
    }

    pub(crate) fn remove_property(&mut self, key: &str) {
        self.properties.retain(|entry| entry.key != key);
    }

    pub(crate) fn fold(&mut self) {
        self.menu_requested = None;
        self.menu = None;
    }

    pub(crate) fn ask(&mut self, key: &str) {
        if key == names::CURRENT_INPUT_METHOD || key.starts_with(names::INPUT_METHOD_ENTRY) {
            return;
        }
        self.menu_requested = Some(key.to_owned());
        self.menu = None;
    }

    pub(crate) fn exec_menu(&mut self, entries: &[String]) {
        let entries = parsed(entries);
        let lists_input_methods =
            !entries.is_empty() && entries.iter().all(|entry| entry.input_method().is_some());
        if lists_input_methods {
            self.input_methods = entries;
            return;
        }
        if let Some(key) = self.menu_requested.take() {
            self.menu = Some(Menu { key, entries });
        }
    }

    pub(crate) fn enable(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub(crate) fn forget(&mut self) {
        *self = State::default();
    }

    fn clamp_focus(&mut self) {
        self.candidates.focused = self
            .candidates
            .focused
            .filter(|at| *at < self.candidates.entries.len());
    }
}

fn parsed(properties: &[String]) -> Vec<Property> {
    properties
        .iter()
        .filter_map(|property| Property::parse(property))
        .collect()
}
