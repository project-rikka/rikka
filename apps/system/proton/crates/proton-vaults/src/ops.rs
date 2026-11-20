use rikka_proton_pass::{
    AliasOptions, ItemFlagsRequest, ItemId, ItemTarget, NewAlias, NewItem, PassClient, ShareId,
    Vault as VaultContent, VaultColor, VaultDisplayPreferences, VaultIcon,
};

use crate::decrypted_item::DecryptedItem;
use crate::draft::Draft;
use crate::entry::Entry;
use crate::error::{Error, Result};
use crate::history::{HistoryEntry, history_entries};
use crate::opened::OpenedItem;
use crate::store::Vault;

pub async fn vaults(client: &PassClient) -> Result<Vec<Vault>> {
    let shares = client.shares().await?;
    Ok(shares.iter().filter_map(Vault::from_share).collect())
}

pub async fn open_item(client: &PassClient, share: &ShareId, id: &ItemId) -> Result<DecryptedItem> {
    let item = client.item(share, id).await?;
    Ok(DecryptedItem::from_item(&item))
}

pub async fn create_item(client: &PassClient, draft: &Draft) -> Result<DecryptedItem> {
    if !draft.is_ready() {
        return Err(Error::NoTitle);
    }
    let item = client
        .create_item(&draft.share, NewItem::new(draft.item()))
        .await?;

    Ok(DecryptedItem::from_item(&item))
}

pub async fn update_item(client: &PassClient, draft: &Draft) -> Result<DecryptedItem> {
    if !draft.is_ready() {
        return Err(Error::NoTitle);
    }
    let id = draft.id.clone().ok_or(Error::NoTitle)?;
    let current = client.item(&draft.share, &id).await?;
    let item = client.update_item(&current, &draft.item()).await?;

    Ok(DecryptedItem::from_item(&item))
}

pub async fn trash_items(client: &PassClient, entries: &[&Entry]) -> Result<()> {
    for (share, targets) in grouped(entries) {
        client.trash_items(&share, targets).await?;
    }
    Ok(())
}

pub async fn restore_items(client: &PassClient, entries: &[&Entry]) -> Result<()> {
    for (share, targets) in grouped(entries) {
        client.untrash_items(&share, targets).await?;
    }
    Ok(())
}

pub async fn delete_items(client: &PassClient, entries: &[&Entry]) -> Result<()> {
    for (share, targets) in grouped(entries) {
        client.delete_items(&share, targets).await?;
    }
    Ok(())
}

pub async fn set_pinned(
    client: &PassClient,
    share: &ShareId,
    id: &ItemId,
    pinned: bool,
) -> Result<DecryptedItem> {
    let item = match pinned {
        true => client.pin_item(share, id).await?,
        false => client.unpin_item(share, id).await?,
    };

    Ok(DecryptedItem::from_item(&item))
}

pub async fn set_monitored(
    client: &PassClient,
    share: &ShareId,
    id: &ItemId,
    monitored: bool,
) -> Result<DecryptedItem> {
    let item = client
        .set_item_flags(
            share,
            id,
            ItemFlagsRequest::new().skip_health_check(!monitored),
        )
        .await?;

    Ok(DecryptedItem::from_item(&item))
}

pub async fn history(
    client: &PassClient,
    share: &ShareId,
    id: &ItemId,
) -> Result<Vec<HistoryEntry>> {
    let revisions = client.item_revisions(share, id).await?;

    Ok(history_entries(
        revisions.iter().map(OpenedItem::from_item).collect(),
    ))
}

pub async fn create_vault(
    client: &PassClient,
    name: &str,
    description: &str,
    icon: i32,
    color: i32,
) -> Result<Vault> {
    let content = VaultContent {
        name: name.trim().to_owned(),
        description: description.to_owned(),
        display: Some(VaultDisplayPreferences { icon, color }),
    };
    let share = client
        .create_vault(rikka_proton_pass::NewVault::from_content(content))
        .await?;

    Vault::from_share(&share).ok_or(Error::NoVault)
}

pub async fn update_vault(
    client: &PassClient,
    share: &ShareId,
    name: &str,
    description: &str,
    icon: i32,
    color: i32,
) -> Result<Vault> {
    let content = VaultContent {
        name: name.trim().to_owned(),
        description: description.to_owned(),
        display: Some(VaultDisplayPreferences { icon, color }),
    };
    let updated = client.update_vault(share, &content).await?;

    Vault::from_share(&updated).ok_or(Error::NoVault)
}

pub async fn delete_vault(client: &PassClient, share: &ShareId) -> Result<()> {
    client.delete_vault(share).await?;
    Ok(())
}

pub async fn alias_options(client: &PassClient, share: &ShareId) -> Result<AliasOptions> {
    Ok(client.alias_options(share).await?)
}

pub async fn create_alias(
    client: &PassClient,
    draft: &Draft,
    prefix: &str,
    suffix: &str,
    mailboxes: &[i64],
) -> Result<DecryptedItem> {
    if !draft.is_ready() {
        return Err(Error::NoTitle);
    }
    if suffix.is_empty() {
        return Err(Error::NoSuffix);
    }

    let alias = NewAlias::new(prefix.trim(), suffix, NewItem::new(draft.item()))
        .with_mailboxes(mailboxes.iter().copied());
    let item = client.create_alias(&draft.share, alias).await?;

    Ok(DecryptedItem::from_item(&item))
}

#[must_use]
pub fn icons() -> Vec<i32> {
    (VaultIcon::Icon1 as i32..=VaultIcon::Icon30 as i32).collect()
}

#[must_use]
pub fn colors() -> Vec<i32> {
    (VaultColor::Color1 as i32..=VaultColor::Color10 as i32).collect()
}

fn grouped(entries: &[&Entry]) -> Vec<(ShareId, Vec<ItemTarget>)> {
    let mut targets: Vec<(ShareId, Vec<ItemTarget>)> = Vec::new();

    for entry in entries {
        let target = ItemTarget::new(entry.id.clone(), entry.revision);
        match targets.iter_mut().find(|(share, _)| share == &entry.share) {
            Some((_, targets)) => targets.push(target),
            None => targets.push((entry.share.clone(), vec![target])),
        }
    }

    targets
}
