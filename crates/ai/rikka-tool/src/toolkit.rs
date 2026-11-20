use rikka_sync::RwLock;
use std::sync::Arc;

use crate::function::Function;

#[derive(Clone, Debug)]
pub struct ToolKit {
    name: String,
    description: String,
    instructions: String,
    functions: Vec<Function>,
}

impl ToolKit {
    #[must_use]
    pub fn new(name: impl Into<String>) -> ToolKit {
        ToolKit {
            name: name.into(),
            description: String::new(),
            instructions: String::new(),
            functions: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_description(mut self, description: impl Into<String>) -> ToolKit {
        self.description = description.into();
        self
    }

    #[must_use]
    pub fn with_instructions(mut self, instructions: impl Into<String>) -> ToolKit {
        self.instructions = instructions.into();
        self
    }

    #[must_use]
    pub fn with_functions(mut self, functions: impl IntoIterator<Item = Function>) -> ToolKit {
        self.functions.extend(functions);
        self
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn description(&self) -> &str {
        &self.description
    }

    #[must_use]
    pub fn instructions(&self) -> &str {
        &self.instructions
    }

    #[must_use]
    pub fn functions(&self) -> &[Function] {
        &self.functions
    }
}

#[derive(Clone, Debug, Default)]
pub struct ToolKitSet {
    kits: Arc<RwLock<Arc<[ToolKit]>>>,
}

impl ToolKitSet {
    #[must_use]
    pub fn new() -> ToolKitSet {
        ToolKitSet::default()
    }

    pub fn replace(&self, kits: impl IntoIterator<Item = ToolKit>) {
        let kits: Arc<[ToolKit]> = kits.into_iter().collect();
        *self.kits.write() = kits;
    }

    #[must_use]
    pub fn kits(&self) -> Arc<[ToolKit]> {
        Arc::clone(&self.kits.read())
    }
}

#[derive(Clone, Debug, Default)]
pub struct ToolKitRegistry {
    sets: Arc<[ToolKitSet]>,
}

impl FromIterator<ToolKitSet> for ToolKitRegistry {
    fn from_iter<I: IntoIterator<Item = ToolKitSet>>(sets: I) -> ToolKitRegistry {
        ToolKitRegistry {
            sets: sets.into_iter().collect(),
        }
    }
}

impl ToolKitRegistry {
    #[must_use]
    pub fn new(sets: impl IntoIterator<Item = ToolKitSet>) -> ToolKitRegistry {
        sets.into_iter().collect()
    }

    #[must_use]
    pub fn kits(&self) -> Arc<[ToolKit]> {
        match &*self.sets {
            [] => Arc::from([]),
            [set] => set.kits(),
            sets => sets
                .iter()
                .flat_map(|set| set.kits().iter().cloned().collect::<Vec<ToolKit>>())
                .collect(),
        }
    }

    #[must_use]
    pub fn kit(&self, name: &str) -> Option<ToolKit> {
        self.kits()
            .iter()
            .find(|kit| kit.name.eq_ignore_ascii_case(name))
            .cloned()
    }

    #[must_use]
    pub fn function(&self, name: &str) -> Option<Function> {
        self.kits()
            .iter()
            .flat_map(|kit| kit.functions.iter())
            .find(|function| function.name() == name)
            .cloned()
    }
}
