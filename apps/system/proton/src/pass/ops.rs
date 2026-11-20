use proton_session::Failure;
use proton_vaults::{
    DecryptedItem, Draft, Entry, HistoryEntry, Recalled, Vault, alias_options, create_item,
    create_vault, delete_items, delete_vault, history, load_cached, open_item, restore_items,
    set_monitored, set_pinned, sync_items, trash_items, update_item, update_vault, vaults,
};
use rikka_app::prelude::*;
use rikka_proton_pass::{AliasOptions, ItemId, ShareId, UserEvents};

use crate::pass::Pass;
use crate::pass::history::History;
use crate::pass::vaults::VaultForm;
use crate::workspace::ProductState;

const ITEM: &str = "pass.item";

impl Pass {
    pub(crate) fn refresh(&mut self) {
        let clients = self.clients.clone();
        self.loading = true;
        let known: Vec<ShareId> = self
            .vaults
            .vaults()
            .iter()
            .map(|vault| vault.share.clone())
            .collect();
        for share in &known {
            self.read(share);
        }

        self.spawn_keyed(
            "pass.vaults",
            async move {
                let client = clients.pass()?;
                Ok(vaults(&client).await?)
            },
            |vault: &mut Pass, result: Result<Vec<Vault>, Failure>| match result {
                Ok(vaults) => {
                    let fresh: Vec<ShareId> = vaults
                        .iter()
                        .filter(|listed| vault.vaults.vault(&listed.share).is_none())
                        .map(|vault| vault.share.clone())
                        .collect();
                    vault.vaults.set_vaults(vaults);
                    for share in fresh {
                        vault.read(&share);
                    }
                    vault.loading = false;
                }
                Err(reason) => {
                    vault.loading = false;
                    vault.error.set(reason);
                }
            },
        );
    }

    pub(super) fn ask_choices(&mut self, share: ShareId) {
        let clients = self.clients.clone();
        self.spawn(
            async move {
                let client = clients.pass()?;
                Ok(alias_options(&client, &share).await?)
            },
            |vault: &mut Pass, options: Result<AliasOptions, Failure>| {
                let Some(editor) = vault.editor.as_mut() else {
                    skip_view();
                    return;
                };
                match options {
                    Ok(choices) => editor.took_choices(choices),
                    Err(reason) => editor.failed(reason),
                }
            },
        );
    }

    pub(crate) fn keep_vault(&mut self) {
        let clients = self.clients.clone();
        let Some(form) = self.form.as_mut() else {
            return;
        };
        let (name, description, icon, color) =
            (form.named(), form.description(), form.icon(), form.color());
        if name.is_empty() {
            form.failed(Failure::new(tr!("proton.pass.needs-name").as_str()));
            return;
        }
        let share = form.share().cloned();

        form.working();
        self.spawn(
            async move {
                let client = clients.pass()?;
                match &share {
                    Some(share) => {
                        update_vault(&client, share, &name, &description, icon, color).await
                    }
                    None => create_vault(&client, &name, &description, icon, color).await,
                }
                .map(drop)
                .map_err(Failure::from)
            },
            |vault: &mut Pass, result: Result<(), Failure>| match result {
                Ok(()) => {
                    vault.form = None;
                    vault.refresh();
                }
                Err(reason) => {
                    if let Some(form) = vault.form.as_mut() {
                        form.failed(reason);
                    }
                }
            },
        );
    }

    pub(crate) fn drop_vault(&mut self) {
        let clients = self.clients.clone();
        let Some(share) = self.form.as_ref().and_then(VaultForm::share).cloned() else {
            return;
        };
        self.form = None;
        self.close();

        self.spawn(
            async move {
                let client = clients.pass()?;
                Ok(delete_vault(&client, &share).await?)
            },
            |vault: &mut Pass, done: Result<(), Failure>| vault.acted(done),
        );
    }

    pub(crate) fn keep(&mut self) {
        let clients = self.clients.clone();
        let Some(editor) = self.editor.as_mut() else {
            return;
        };
        let draft = editor.draft();
        if !draft.is_ready() {
            editor.failed(Failure::new(tr!("proton.pass.needs-name").as_str()));
            return;
        }

        let alias = match editor.alias() {
            Ok(alias) => alias,
            Err(reason) => {
                editor.failed(Failure::new(reason.as_str()));
                return;
            }
        };
        editor.working();
        self.spawn(
            async move {
                let client = clients.pass()?;
                let result = match (&alias, draft.is_new()) {
                    (Some(alias), true) => {
                        proton_vaults::create_alias(
                            &client,
                            &draft,
                            &alias.prefix,
                            &alias.suffix,
                            &alias.mailboxes,
                        )
                        .await
                    }
                    (_, true) => create_item(&client, &draft).await,
                    (_, false) => update_item(&client, &draft).await,
                };
                result.map_err(Failure::from)
            },
            |vault: &mut Pass, result: Result<DecryptedItem, Failure>| match result {
                Ok(unlocked) => vault.item_saved(unlocked),
                Err(reason) => {
                    if let Some(editor) = vault.editor.as_mut() {
                        editor.failed(reason);
                    }
                }
            },
        );
    }

    pub(crate) fn erase(&mut self) {
        let clients = self.clients.clone();
        let Some(entry) = self.entry() else {
            return;
        };
        let share = entry.share.clone();
        self.vaults.remove(&share, std::slice::from_ref(&entry.id));
        self.close();

        self.spawn(
            async move {
                let client = clients.pass()?;
                Ok(delete_items(&client, &[&entry]).await?)
            },
            move |vault: &mut Pass, done: Result<(), Failure>| vault.acted(done),
        );
    }

    pub(crate) fn pin(&mut self) {
        let clients = self.clients.clone();
        let Some(opened) = self.opened.as_mut() else {
            return;
        };
        let (share, id) = (opened.share.clone(), opened.id.clone());
        let pinned = !opened.pinned;

        opened.pinned = pinned;
        self.vaults.set_pinned(&share, &id, pinned);
        self.spawn(
            async move {
                let client = clients.pass()?;
                Ok(set_pinned(&client, &share, &id, pinned).await?)
            },
            |vault: &mut Pass, done: Result<DecryptedItem, Failure>| vault.settled(done),
        );
    }

    pub(crate) fn watch(&mut self) {
        let clients = self.clients.clone();
        let Some((share, id)) = self.chosen.clone() else {
            return;
        };
        let monitored = self
            .vaults
            .entry(&share, &id)
            .is_some_and(Entry::is_excluded);

        self.spawn(
            async move {
                let client = clients.pass()?;
                Ok(set_monitored(&client, &share, &id, monitored).await?)
            },
            |vault: &mut Pass, done: Result<DecryptedItem, Failure>| vault.settled(done),
        );
    }

    pub(crate) fn show_history(&mut self) {
        let clients = self.clients.clone();
        let Some((share, id)) = self.chosen.clone() else {
            return;
        };
        self.history = Some(History::working());

        self.spawn_keyed(
            "pass.history",
            async move {
                let client = clients.pass()?;
                Ok(history(&client, &share, &id).await?)
            },
            |vault: &mut Pass, result: Result<Vec<HistoryEntry>, Failure>| {
                let Some(history) = vault.history.as_mut() else {
                    skip_view();
                    return;
                };
                match result {
                    Ok(history_entries) => history.took(history_entries),
                    Err(reason) => history.failed(reason),
                }
            },
        );
    }

    pub(crate) fn restore_moment(&mut self, at: usize) {
        let clients = self.clients.clone();
        let Some(moment) = self
            .history
            .as_ref()
            .and_then(|history| history.moment(at))
            .map(|moment| moment.opened.clone())
        else {
            return;
        };
        let draft = Draft::from_opened(&moment);
        self.history = None;

        self.spawn(
            async move {
                let client = clients.pass()?;
                Ok(update_item(&client, &draft).await?)
            },
            |vault: &mut Pass, done: Result<DecryptedItem, Failure>| match done {
                Ok(unlocked) => vault.item_saved(unlocked),
                Err(reason) => vault.error.set(reason),
            },
        );
    }

    pub(super) fn settled(&mut self, done: Result<DecryptedItem, Failure>) {
        match done {
            Ok(unlocked) => {
                if let Some(opened) = self.opened.as_mut()
                    && opened.id == unlocked.entry.id
                {
                    opened.pinned = unlocked.entry.pinned;
                }
                self.vaults.put(unlocked);
            }
            Err(reason) => self.error.set(reason),
        }
    }

    pub(crate) fn copied(&mut self, value: String) {
        set_text_of(Selection::Clipboard, &value);
        let clients = self.clients.clone();
        let Some((share, id)) = self.chosen.clone() else {
            return;
        };
        let at = jiff::Timestamp::now();
        self.spawn(
            async move {
                let client = clients.pass()?;
                Ok(client.mark_item_used(&share, &id, at).await.map(drop)?)
            },
            |_vault: &mut Pass, _done: Result<(), Failure>| skip_view(),
        );
    }

    pub(super) fn act(&mut self, entry: Entry, back: bool) {
        let clients = self.clients.clone();
        self.spawn(
            async move {
                let client = clients.pass()?;
                match back {
                    true => restore_items(&client, &[&entry]).await,
                    false => trash_items(&client, &[&entry]).await,
                }
                .map_err(Failure::from)
            },
            move |vault: &mut Pass, done: Result<(), Failure>| vault.acted(done),
        );
    }

    pub(super) fn acted(&mut self, done: Result<(), Failure>) {
        match done {
            Ok(()) => self.refresh(),
            Err(reason) => self.error.set(reason),
        }
    }

    pub(super) fn item_saved(&mut self, unlocked: DecryptedItem) {
        let (share, id) = (unlocked.entry.share.clone(), unlocked.entry.id.clone());
        self.vaults.put(unlocked);
        self.editor = None;
        self.chosen = Some((share.clone(), id.clone()));
        self.open(&share, &id);
    }

    pub(super) fn open(&mut self, share: &ShareId, id: &ItemId) {
        if let Some(opened) = self.vaults.opened(share, id) {
            cancel(ITEM);
            self.loading = false;
            self.opened = Some(opened);
            return;
        }

        let clients = self.clients.clone();
        let (share, id) = (share.clone(), id.clone());
        let item_key = (share.clone(), id.clone());
        self.loading = true;

        self.spawn_keyed(
            ITEM,
            async move {
                let client = clients.pass()?;
                Ok(open_item(&client, &share, &id).await?)
            },
            move |vault: &mut Pass, result: Result<DecryptedItem, Failure>| {
                vault.loading = false;
                match result {
                    Ok(unlocked) => {
                        vault.vaults.put(unlocked);
                        if vault.chosen.as_ref() == Some(&item_key) {
                            vault.opened = vault.vaults.opened(&item_key.0, &item_key.1);
                        }
                    }
                    Err(reason) => vault.error.set(reason),
                }
            },
        );
    }

    pub(crate) fn apply_events(&mut self, events: &UserEvents) {
        if !self.started {
            skip_view();
            return;
        }
        let changed_shares = events.changed_shares();
        let unknown = changed_shares
            .iter()
            .any(|share| self.vaults.vault(share).is_none());
        if events.is_full_refresh() || unknown {
            self.refresh();
            return;
        }
        if changed_shares.is_empty() {
            skip_view();
            return;
        }
        for share in &changed_shares {
            self.read(share);
        }
    }

    pub(super) fn read(&mut self, share: &ShareId) {
        let clients = self.clients.clone();
        let share_id = share.clone();

        self.spawn_keyed(
            format!("pass.items.{share_id}"),
            async move {
                let client = clients.pass()?;
                sync_items(&client, &share_id)
                    .await
                    .map(|items| (share_id, items))
                    .map_err(Failure::from)
            },
            |vault: &mut Pass, result: Result<(ShareId, Vec<DecryptedItem>), Failure>| match result
            {
                Ok((share, items)) => {
                    vault.vaults.synced(&share, items);
                    vault.reopen(&share);
                }
                Err(reason) => vault.error.set(reason),
            },
        );
    }

    pub(super) fn recall(&mut self) {
        let clients = self.clients.clone();
        self.loading = true;

        self.spawn(
            async move {
                let client = clients.pass().ok()?;
                load_cached(&client).await.ok().flatten()
            },
            |vault: &mut Pass, recalled: Option<Recalled>| {
                if let Some(Recalled { vaults, lists }) = recalled {
                    vault.vaults.restore(vaults, lists);
                }
                vault.refresh();
            },
        );
    }

    fn reopen(&mut self, share: &ShareId) {
        let Some(shown) = self.opened.as_ref().filter(|opened| &opened.share == share) else {
            return;
        };
        if let Some(fresh) = self.vaults.opened(share, &shown.id)
            && fresh.revision != shown.revision
        {
            self.opened = Some(fresh);
        }
    }
}
