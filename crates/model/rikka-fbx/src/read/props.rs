use std::sync::Arc;

use rikka_model_core::glam::Vec3;
use rustc_hash::FxHashMap;

use crate::document::{Node, Value};

#[derive(Clone, Debug)]
pub(crate) struct Prop {
    pub values: Vec<Value>,
}

impl Prop {
    pub(crate) fn number(&self, index: usize) -> Option<f64> {
        self.values.get(index)?.as_number()
    }

    pub(crate) fn text(&self) -> Option<&str> {
        self.values.first()?.as_text()
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Props {
    props: FxHashMap<String, Prop>,
    template: Option<Arc<Props>>,
}

impl Props {
    pub(crate) fn read(node: &Node) -> Self {
        let (block, skip) = match node.child("Properties70") {
            Some(block) => (block, 4),
            None => match node.child("Properties60") {
                Some(block) => (block, 3),
                None => return Self::default(),
            },
        };
        let mut props = FxHashMap::default();
        for entry in block.children.iter() {
            let Some(name) = entry.text(0) else {
                continue;
            };
            props.insert(
                name.to_owned(),
                Prop {
                    values: entry.values.iter().skip(skip).cloned().collect(),
                },
            );
        }
        Self {
            props,
            template: None,
        }
    }

    pub(crate) fn with_template(mut self, template: Option<Arc<Props>>) -> Self {
        self.template = template;
        self
    }

    pub(crate) fn get(&self, name: &str) -> Option<&Prop> {
        match self.props.get(name) {
            Some(prop) => Some(prop),
            None => self.template.as_ref()?.get(name),
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.props.is_empty()
    }

    pub(crate) fn names(&self) -> impl Iterator<Item = &str> {
        self.props.keys().map(String::as_str)
    }

    pub(crate) fn number(&self, name: &str) -> Option<f64> {
        self.get(name)?.number(0)
    }

    pub(crate) fn real(&self, name: &str, fallback: f32) -> f32 {
        self.number(name).map_or(fallback, |value| value as f32)
    }

    pub(crate) fn integer(&self, name: &str) -> Option<i64> {
        self.get(name)?.values.first()?.as_integer()
    }

    pub(crate) fn flag(&self, name: &str) -> Option<bool> {
        Some(self.integer(name)? != 0)
    }

    pub(crate) fn text(&self, name: &str) -> Option<&str> {
        self.get(name)?.text()
    }

    pub(crate) fn vec3(&self, name: &str, fallback: Vec3) -> Vec3 {
        let Some(prop) = self.get(name) else {
            return fallback;
        };
        Vec3::new(
            prop.number(0).unwrap_or(f64::from(fallback.x)) as f32,
            prop.number(1).unwrap_or(f64::from(fallback.y)) as f32,
            prop.number(2).unwrap_or(f64::from(fallback.z)) as f32,
        )
    }
}
