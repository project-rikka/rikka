use std::collections::BTreeSet;

use rikka_keyring_vault::Keyring as Vault;
use zbus::zvariant::OwnedObjectPath;

use crate::fault::{Answer, Fault};
use crate::paths::{self, Target};
use crate::state::{Change, State};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Prompt {
    owner: String,
    objects: Vec<Requested>,
    shown: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Requested {
    object: OwnedObjectPath,
    collection: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Unlocking {
    pub(crate) unlocked: Vec<OwnedObjectPath>,
    pub(crate) prompt: OwnedObjectPath,
}

impl State {
    pub(crate) fn unlock(&mut self, objects: Vec<OwnedObjectPath>, owner: &str) -> Unlocking {
        let before = self.locked_names();
        self.settle(&before);
        let mut unlocked = Vec::new();
        let mut waiting = Vec::new();
        for object in objects {
            let Some(collection) = self.collection_of(&object) else {
                continue;
            };
            match self
                .collections
                .get(&collection)
                .is_some_and(|vault| !vault.is_locked())
            {
                true => unlocked.push(object),
                false => waiting.push(Requested { object, collection }),
            }
        }
        if waiting.is_empty() {
            return Unlocking {
                unlocked,
                prompt: paths::none(),
            };
        }
        let serial = self.next_serial();
        let prompt = paths::prompt(serial);
        self.prompts.insert(
            prompt.clone(),
            Prompt {
                owner: owner.to_owned(),
                objects: waiting,
                shown: false,
            },
        );
        self.changes.push(Change::PromptOpened(prompt.clone()));
        Unlocking { unlocked, prompt }
    }

    pub(crate) fn show_prompt(&mut self, path: &OwnedObjectPath) -> Answer<()> {
        let prompt = self
            .prompts
            .get_mut(path)
            .ok_or_else(|| Fault::no_object(path.as_str()))?;
        if !prompt.shown {
            prompt.shown = true;
            self.changes.push(Change::Keyring);
        }
        self.complete_satisfied();
        Ok(())
    }

    pub(crate) fn dismiss_prompt(&mut self, path: &OwnedObjectPath) -> Answer<()> {
        let prompt = self
            .prompts
            .remove(path)
            .ok_or_else(|| Fault::no_object(path.as_str()))?;
        self.finish(path.clone(), &prompt, true, Vec::new());
        Ok(())
    }

    pub(crate) fn asking(&self) -> Vec<OwnedObjectPath> {
        let mut asking: Vec<OwnedObjectPath> = self
            .prompts
            .iter()
            .filter(|(_, prompt)| prompt.shown)
            .map(|(path, _)| path.clone())
            .collect();
        asking.sort_by(|left, right| left.as_str().cmp(right.as_str()));
        asking
    }

    pub(crate) fn dismiss_prompts_of(&mut self, owner: &str) {
        let gone: Vec<OwnedObjectPath> = self
            .prompts
            .iter()
            .filter(|(_, prompt)| prompt.owner == owner)
            .map(|(path, _)| path.clone())
            .collect();
        for path in gone {
            if let Some(prompt) = self.prompts.remove(&path) {
                self.finish(path, &prompt, true, Vec::new());
            }
        }
    }

    pub(super) fn complete_satisfied(&mut self) {
        let open: BTreeSet<String> = self
            .collections
            .iter()
            .filter(|(_, vault)| !Vault::is_locked(vault))
            .map(|(name, _)| name.clone())
            .collect();
        let satisfied: Vec<OwnedObjectPath> = self
            .prompts
            .iter()
            .filter(|(_, prompt)| {
                prompt
                    .objects
                    .iter()
                    .all(|requested| open.contains(&requested.collection))
            })
            .map(|(path, _)| path.clone())
            .collect();
        for path in satisfied {
            if let Some(prompt) = self.prompts.remove(&path) {
                let result = prompt
                    .objects
                    .iter()
                    .map(|requested| requested.object.clone())
                    .collect();
                self.finish(path, &prompt, false, result);
            }
        }
    }

    fn finish(
        &mut self,
        path: OwnedObjectPath,
        prompt: &Prompt,
        dismissed: bool,
        result: Vec<OwnedObjectPath>,
    ) {
        self.changes.push(Change::PromptCompleted {
            path,
            dismissed,
            result,
        });
        if prompt.shown {
            self.changes.push(Change::Keyring);
        }
    }

    pub(crate) fn collection_of(&self, object: &OwnedObjectPath) -> Option<String> {
        let name = match paths::target(object)? {
            Target::Collection(name) | Target::Item(name, _) => name,
            Target::Alias(alias) => self.resolve_alias(&alias)?,
        };
        self.exists(&name).then_some(name)
    }
}
