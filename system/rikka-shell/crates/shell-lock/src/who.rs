use std::path::{Path, PathBuf};

use rikka_dirs::passwd::Entry;

const ACCOUNTS_SERVICE: &str = "/var/lib/AccountsService/icons";
const FACES: [&str; 2] = [".face", ".face.icon"];

#[must_use]
pub(crate) fn face(account: &Entry) -> Option<PathBuf> {
    std::iter::once(Path::new(ACCOUNTS_SERVICE).join(&account.name))
        .chain(FACES.iter().map(|face| account.home.join(face)))
        .find(|candidate| candidate.is_file())
}
