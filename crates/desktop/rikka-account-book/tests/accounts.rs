use rikka_account_book::{Accounts, Record, Vault};
use rikka_secrets::blocking::Secrets;
use rikka_test_keyring::TestKeyring;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Saved {
    key: String,
    service: String,
    id: String,
    name: String,
}

impl Record for Saved {
    const VAULT: Vault = Vault::new("os.rikka.AccountsTest");

    fn key(&self) -> &str {
        &self.key
    }

    fn is_same_account(&self, other: &Saved) -> bool {
        self.service == other.service && self.id == other.id
    }
}

fn saved(service: &str, id: &str, name: &str) -> impl FnOnce(String) -> Saved {
    let (service, id, name) = (service.to_owned(), id.to_owned(), name.to_owned());
    move |key| Saved {
        key,
        service,
        id,
        name,
    }
}

#[test]
fn committed_account_survives_reopen() {
    let root = tempfile::tempdir().unwrap();
    let mut accounts = Accounts::<Saved>::open(root.path()).unwrap();
    let pending = accounts.reserve().unwrap();
    let folder = pending.folder().to_owned();

    let committed = accounts.commit(pending, saved("a", "1", "Me")).unwrap();

    assert!(folder.is_dir());
    assert!(committed.replaced.is_none());
    let reopened = Accounts::<Saved>::open(root.path()).unwrap();
    assert_eq!(reopened.current(), Some(&committed.saved));
    assert_eq!(reopened.folder(&committed.saved.key), folder);
}

#[test]
fn abandoned_folder_is_removed() {
    let Some(_keyring) = TestKeyring::unlocked() else {
        return;
    };
    let root = tempfile::tempdir().unwrap();
    let accounts = Accounts::<Saved>::open(root.path()).unwrap();
    let pending = accounts.reserve().unwrap();
    let folder = pending.folder().to_owned();

    pending.abandon().wait().unwrap();

    assert!(!folder.exists());
    assert!(accounts.all().is_empty());
}

#[test]
fn same_account_replaces_entry_folder_and_secrets() {
    let Some(_keyring) = TestKeyring::unlocked() else {
        return;
    };
    let root = tempfile::tempdir().unwrap();
    let mut accounts = Accounts::<Saved>::open(root.path()).unwrap();
    let first = accounts
        .commit(accounts.reserve().unwrap(), saved("a", "1", "A"))
        .unwrap()
        .saved;
    let other = accounts
        .commit(accounts.reserve().unwrap(), saved("b", "1", "A"))
        .unwrap()
        .saved;
    let secrets = Secrets::shared().unwrap();
    let cookies = Saved::VAULT
        .secret(&accounts.folder(&first.key))
        .kind("cookies");
    secrets
        .store(cookies.entry(), "sign-in", b"notreal")
        .unwrap();

    let again = accounts
        .commit(accounts.reserve().unwrap(), saved("a", "1", "A2"))
        .unwrap();
    again.replaced.unwrap().wait().unwrap();

    let listed: Vec<_> = accounts
        .all()
        .iter()
        .map(|known| (known.service.as_str(), known.name.as_str()))
        .collect();
    assert_eq!(listed, vec![("a", "A2"), ("b", "A")]);
    assert!(accounts.folder(&other.key).is_dir());
    assert!(!accounts.folder(&first.key).exists());
    assert!(secrets.lookup(cookies.entry()).unwrap().is_none());
    assert_eq!(accounts.current(), Some(&again.saved));
}

#[test]
fn forgetting_current_selects_first_remaining() {
    let Some(_keyring) = TestKeyring::unlocked() else {
        return;
    };
    let root = tempfile::tempdir().unwrap();
    let mut accounts = Accounts::<Saved>::open(root.path()).unwrap();
    let first = accounts
        .commit(accounts.reserve().unwrap(), saved("a", "1", "A"))
        .unwrap()
        .saved;
    let second = accounts
        .commit(accounts.reserve().unwrap(), saved("a", "2", "B"))
        .unwrap()
        .saved;
    accounts.choose(&first.key).unwrap();
    let secrets = Secrets::shared().unwrap();
    let sign_in = Saved::VAULT.secret(&accounts.folder(&first.key));
    secrets
        .store(sign_in.entry(), "sign-in", b"notarealtoken")
        .unwrap();

    accounts
        .forget(&first.key)
        .unwrap()
        .unwrap()
        .wait()
        .unwrap();
    assert_eq!(accounts.current(), Some(&second));
    assert!(!accounts.folder(&first.key).exists());
    assert!(secrets.lookup(sign_in.entry()).unwrap().is_none());

    accounts
        .forget(&second.key)
        .unwrap()
        .unwrap()
        .wait()
        .unwrap();
    assert_eq!(accounts.current(), None);
}
