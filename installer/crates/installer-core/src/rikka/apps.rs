use rikka_nix_edit::Set;

use crate::answers::Answers;

pub(crate) fn body(answers: &Answers) -> Set {
    answers.apps.iter().fold(Set::new(), |set, id| {
        set.bind(["rikka", "apps", id.as_str(), "enable"], true)
    })
}
