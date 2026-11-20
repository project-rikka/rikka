use std::path::Path;

use jiff::{Timestamp, ToSpan as _};
use rikka_proton_drive_sync_state::{
    Changes, DraftBlock, DraftProgress, Error, Kind, Operation, Root, Side, Store, SyncedItem,
    Target, UploadDraft,
};

use crate::support::{LOCAL_ROOT, REMOTE_ROOT, local_item};

fn root(local_path: &Path) -> Root {
    Root {
        local_path: local_path.to_owned(),
        target: Target::MyFiles("Work/Projects".to_owned()),
        volume: "volume".to_owned(),
        share: "share".to_owned(),
        remote_root: REMOTE_ROOT.to_owned(),
        local_root: LOCAL_ROOT,
    }
}

fn upload(local: u64) -> Operation {
    Operation::Upload {
        local,
        parent: REMOTE_ROOT.to_owned(),
        name: format!("{local}.txt"),
        sha1: "abc".to_owned(),
    }
}

#[test]
fn interrupted_operations_are_recovered() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("sync.sqlite");
    let partial = folder.path().join(".~rikka-sync-0001.part");
    let planned = vec![upload(20), upload(21)];

    let id = {
        let mut store = Store::open(&path).unwrap();
        let id = store.add_root(&root(folder.path())).unwrap();
        let entries = store.schedule(id, &planned).unwrap();
        assert_eq!(entries.len(), 2);
        store.begin(entries[0].id, Some(&partial)).unwrap();
        let attempts = store
            .fail(
                entries[1].id,
                "the network went away",
                Timestamp::now() + 1.hour(),
            )
            .unwrap();
        assert_eq!(attempts, 1);
        let progress: Vec<DraftProgress> = std::iter::once(drafted())
            .chain(sent_blocks().into_iter().map(DraftProgress::BlockUploaded))
            .collect();
        store
            .record_draft_progress(entries[1].id, &progress)
            .unwrap();

        let second = Store::open(&path).unwrap_err();
        assert!(matches!(second, Error::Busy), "{second}");
        id
    };

    let mut store = Store::open(&path).unwrap();
    let interrupted_entries = store.interrupted_entries().unwrap();
    assert_eq!(interrupted_entries.len(), 1);
    assert_eq!(interrupted_entries[0].operation, upload(20));
    assert_eq!(
        interrupted_entries[0].artifact.as_deref(),
        Some(partial.as_path())
    );
    store.release(interrupted_entries[0].id).unwrap();

    let resumed = store.schedule(id, &planned).unwrap();
    assert_eq!(
        resumed
            .iter()
            .map(|entry| (&entry.operation, entry.attempts))
            .collect::<Vec<_>>(),
        vec![(&upload(20), 0), (&upload(21), 1)]
    );
    assert_eq!(
        store.draft(resumed[1].id).unwrap(),
        Some(UploadDraft {
            link: "draft".to_owned(),
            revision: "revision".to_owned(),
            blocks: sent_blocks(),
        })
    );
    store
        .record_draft_progress(resumed[1].id, &[drafted()])
        .unwrap();
    assert_eq!(
        store
            .draft(resumed[1].id)
            .unwrap()
            .map(|draft| draft.blocks.len()),
        Some(2)
    );
    assert!(
        !resumed[1].is_due(Timestamp::now()),
        "failure did not back off"
    );
    assert_eq!(resumed[1].error.as_deref(), Some("the network went away"));

    let uploaded = local_item(20, LOCAL_ROOT, "20.txt", Kind::File, Some("abc"));
    let changes = Changes {
        local: vec![uploaded.clone()],
        synced: vec![SyncedItem {
            id: 1,
            kind: Kind::File,
            local: 20,
            local_parent: LOCAL_ROOT,
            local_name: "20.txt".to_owned(),
            remote: "uploaded".to_owned(),
            remote_parent: REMOTE_ROOT.to_owned(),
            remote_name: "20.txt".to_owned(),
            sha1: Some("abc".to_owned()),
            size: 3,
            revision: Some("r1".to_owned()),
        }],
        ..Changes::default()
    };
    store.finish(id, resumed[0].id, &changes).unwrap();
    drop(store);

    let mut store = Store::open(&path).unwrap();
    let (_, recovered_root) = store.roots().unwrap().into_iter().next().unwrap();
    assert_eq!(recovered_root, root(folder.path()));
    let state = store.load(id, &recovered_root).unwrap();
    assert_eq!(state.local().get(&20), Some(&uploaded));
    assert_eq!(state.synced().get(&1), Some(&changes.synced[0]));
    assert_eq!(
        store
            .entries(id)
            .unwrap()
            .into_iter()
            .map(|entry| entry.operation)
            .collect::<Vec<_>>(),
        vec![upload(21)]
    );

    store.schedule(id, &[]).unwrap();
    assert!(store.entries(id).unwrap().is_empty());
    assert_eq!(store.draft(resumed[1].id).unwrap(), None);
    store.clear(id, Side::Local).unwrap();
    let state = store.load(id, &recovered_root).unwrap();
    assert!(state.local().is_empty() && state.synced().is_empty());
}
fn drafted() -> DraftProgress {
    DraftProgress::Created {
        link: "draft".to_owned(),
        revision: "revision".to_owned(),
    }
}

fn sent_blocks() -> Vec<DraftBlock> {
    (1..=2)
        .map(|index| DraftBlock {
            index,
            hash: [index as u8; 32],
            size: 4 * 1024 * 1024,
            digest: [0xa0 + index as u8; 32],
        })
        .collect()
}
