use jiff::Timestamp;
use rikka_proton_authenticator::{
    Entry, EntryState, LocalEntry, RemoteEntry, SyncAction, SyncOrder, SyncPlan,
};

const NOW: i64 = 1_700_000_000;
const LATER: i64 = NOW + 1_000;
const LATEST: i64 = NOW + 2_000;

fn timestamp(second: i64) -> Timestamp {
    Timestamp::from_second(second).unwrap()
}

fn entry(issuer: &str) -> Entry {
    entry_with_id("one", issuer)
}

fn entry_with_id(id: &str, issuer: &str) -> Entry {
    Entry::from_uri(
        id,
        &format!("otpauth://totp/person?secret=JBSWY3DPEHPK3PXP&issuer={issuer}"),
    )
    .unwrap()
}

fn remote(issuer: &str, modified_at: i64) -> RemoteEntry {
    RemoteEntry {
        remote_id: "remote-one".to_owned(),
        revision: 2,
        entry: entry(issuer),
        position: Some(0),
        created_at: timestamp(NOW),
        modified_at: timestamp(modified_at),
    }
}

fn local(issuer: &str, state: EntryState, edited_at: Option<i64>) -> LocalEntry {
    let local_entry = LocalEntry::new(entry(issuer), state, 0, timestamp(NOW));
    match edited_at {
        Some(edited_at) => local_entry.with_edited_at(timestamp(edited_at)),
        None => local_entry,
    }
}

#[test]
fn sync_plan_keeps_newest_entries() {
    let cases = [
        (
            Some(remote("Proton", NOW)),
            None,
            Some((SyncAction::Upsert, "Proton")),
        ),
        (
            None,
            Some(local("Proton", EntryState::Synced, None)),
            Some((SyncAction::DeleteLocal, "Proton")),
        ),
        (
            None,
            Some(local("Proton", EntryState::PendingSync, None)),
            Some((SyncAction::Push, "Proton")),
        ),
        (
            Some(remote("Proton", NOW)),
            Some(local("Proton", EntryState::PendingDelete, None)),
            Some((SyncAction::DeleteEverywhere, "Proton")),
        ),
        (
            Some(remote("Proton", NOW)),
            Some(local("Proton", EntryState::Synced, None)),
            None,
        ),
        (
            Some(remote("Proton", LATER)),
            Some(local("Other", EntryState::Synced, None)),
            Some((SyncAction::Upsert, "Proton")),
        ),
        (
            Some(remote("Proton", NOW)),
            Some(local("Other", EntryState::PendingSync, Some(LATEST))),
            Some((SyncAction::Push, "Other")),
        ),
        (
            Some(remote("Proton", LATEST)),
            Some(local("Other", EntryState::PendingSync, Some(LATER))),
            Some((SyncAction::Upsert, "Proton")),
        ),
        (
            Some(remote("Proton", LATEST)),
            Some(local(
                "Other",
                EntryState::PendingSync,
                Some(LATEST + 1_000),
            )),
            Some((SyncAction::Push, "Other")),
        ),
        (
            Some(remote("Proton", LATER)),
            Some(local("Other", EntryState::PendingSync, None)),
            Some((SyncAction::Upsert, "Proton")),
        ),
        (
            Some(remote("Proton", NOW)),
            Some(local("Proton", EntryState::PendingSync, None)),
            Some((SyncAction::Upsert, "Proton")),
        ),
    ];
    assert!(SyncPlan::compute(&[], &[]).is_empty());
    for (index, (remote, local, expected)) in cases.into_iter().enumerate() {
        let plan = SyncPlan::compute(remote.as_slice(), local.as_slice());
        let steps: Vec<_> = plan
            .steps()
            .iter()
            .map(|step| (step.action, step.entry.issuer().unwrap()))
            .collect();
        assert_eq!(steps, Vec::from_iter(expected), "case {index}");
        if let (Some(step), Some(remote)) = (plan.steps().first(), &remote) {
            assert_eq!(step.remote_id.as_deref(), Some(remote.remote_id.as_str()));
            assert_eq!(step.revision, Some(remote.revision));
        }
    }
}

fn remote_entry(id: &str, position: u32, modified_at: i64) -> RemoteEntry {
    RemoteEntry {
        remote_id: format!("remote-{id}"),
        revision: 1,
        entry: entry_with_id(id, "Proton"),
        position: Some(position),
        created_at: timestamp(NOW),
        modified_at: timestamp(modified_at),
    }
}

fn local_entry(id: &str, state: EntryState, position: u32, modified_at: i64) -> LocalEntry {
    LocalEntry::new(
        entry_with_id(id, "Proton"),
        state,
        position,
        timestamp(modified_at),
    )
}

#[test]
fn sync_order_keeps_newer_positions() {
    let cases = [
        (
            remote_entry("one", 3, LATER),
            local_entry("one", EntryState::Synced, 3, NOW),
            (3, NOW),
        ),
        (
            remote_entry("one", 4, LATER),
            local_entry("one", EntryState::PendingSync, 1, NOW),
            (1, NOW),
        ),
        (
            remote_entry("one", 5, NOW),
            local_entry("one", EntryState::Synced, 2, LATER),
            (2, LATER),
        ),
        (
            remote_entry("one", 5, LATEST),
            local_entry("one", EntryState::Synced, 2, NOW),
            (5, LATEST),
        ),
    ];
    for (index, (remote, local, (position, modified_at))) in cases.into_iter().enumerate() {
        let order = SyncOrder::compute(&[remote], &[local]);
        let placement = &order.placements()[0];
        assert_eq!(placement.id, "one", "case {index}");
        assert_eq!(placement.remote_id, "remote-one", "case {index}");
        assert_eq!(placement.position, position, "case {index}");
        assert_eq!(
            placement.modified_at,
            timestamp(modified_at),
            "case {index}"
        );
    }

    let remote = [
        remote_entry("first", 0, NOW),
        remote_entry("second", 1, NOW),
        remote_entry("third", 2, NOW),
        remote_entry("foreign_content", 3, NOW),
    ];
    let local = [
        local_entry("first", EntryState::PendingSync, 2, LATER),
        local_entry("second", EntryState::Synced, 1, NOW),
        local_entry("third", EntryState::PendingSync, 0, LATER),
    ];
    let order = SyncOrder::compute(&remote, &local);
    assert_eq!(order.starting_position(), Some(0));
    assert_eq!(
        order.remote_ids(),
        ["remote-third", "remote-second", "remote-first"]
    );
    assert!(SyncOrder::compute(&remote, &[]).is_empty());
}
