use std::collections::HashMap;

use rikka_input::Chord;
use rikka_session::shortcuts::{Group, Shortcut};
use rikka_settings::Value;

use crate::domain::Domain;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Binding {
    name: String,
    summary: String,
    chord: Option<Chord>,
    default: Option<Chord>,
    group: Option<Group>,
    shared: bool,
}

impl Binding {
    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    pub(crate) fn summary(&self) -> &str {
        &self.summary
    }

    pub(crate) fn chord(&self) -> Option<Chord> {
        self.chord
    }

    pub(crate) fn is_default(&self) -> bool {
        self.chord == self.default
    }

    pub(crate) fn shared(&self) -> bool {
        self.shared
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Bindings {
    bindings: Vec<Binding>,
    groups: Vec<Option<Group>>,
}

impl Bindings {
    pub(crate) fn read(domain: &Domain) -> Bindings {
        let Some(schema) = domain.schema() else {
            return Bindings::default();
        };
        Bindings::new(
            schema
                .keys()
                .iter()
                .map(|key| Binding {
                    name: key.name().to_owned(),
                    summary: key.summary_text().unwrap_or_default().to_owned(),
                    chord: chord(domain.get::<String>(key.name()).as_deref()),
                    default: match key.default_value() {
                        Value::String(text) => chord(Some(text)),
                        _ => None,
                    },
                    group: key.name().parse::<Shortcut>().ok().map(Shortcut::group),
                    shared: false,
                })
                .collect(),
        )
    }

    fn new(mut bindings: Vec<Binding>) -> Bindings {
        let mut uses: HashMap<Chord, usize> = HashMap::new();
        for binding in &bindings {
            if let Some(chord) = binding.chord {
                *uses.entry(chord).or_default() += 1;
            }
        }
        for binding in &mut bindings {
            binding.shared = binding
                .chord
                .is_some_and(|chord| uses.get(&chord).is_some_and(|count| *count > 1));
        }
        let groups = Group::ALL
            .into_iter()
            .map(Some)
            .chain([None])
            .filter(|group| bindings.iter().any(|binding| binding.group == *group))
            .collect();
        Bindings { bindings, groups }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }

    pub(crate) fn groups(&self) -> &[Option<Group>] {
        &self.groups
    }

    pub(crate) fn in_group(&self, group: Option<Group>) -> impl Iterator<Item = &Binding> {
        self.bindings
            .iter()
            .filter(move |binding| binding.group == group)
    }

    pub(crate) fn named(&self, name: &str) -> Option<&Binding> {
        self.bindings.iter().find(|binding| binding.name == name)
    }

    pub(crate) fn also_bound(&self, chord: Chord, besides: &str) -> Option<&Binding> {
        self.bindings
            .iter()
            .find(|binding| binding.name != besides && binding.chord == Some(chord))
    }
}

fn chord(text: Option<&str>) -> Option<Chord> {
    text.filter(|text| !text.is_empty())
        .and_then(|text| text.parse().ok())
}
