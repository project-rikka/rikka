use jiff::civil::date;
use rikka_proton_drive_sync_state::{Conflict, Context, Kind, Operation, reconcile};

use crate::support::{LOCAL_ROOT, REMOTE_ROOT, Tree, local_item};

struct Case {
    name: &'static str,
    change: fn(&mut Tree),
    expected: Vec<Operation>,
}

fn upload(local: u64, parent: &str, name: &str, sha1: &str) -> Operation {
    Operation::Upload {
        local,
        parent: parent.to_owned(),
        name: name.to_owned(),
        sha1: sha1.to_owned(),
    }
}

fn download(remote: &str, parent: u64, name: &str, revision: &str) -> Operation {
    Operation::Download {
        remote: remote.to_owned(),
        parent,
        name: name.to_owned(),
        revision: revision.to_owned(),
    }
}

fn edited_remotely(tree: &mut Tree, id: &str, sha1: &str) {
    let remote = tree.remote(id);
    remote.revision = Some("r2".to_owned());
    remote.sha1 = Some(sha1.to_owned());
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "unchanged",
            change: |_| {},
            expected: vec![],
        },
        Case {
            name: "new_local_file_uploads",
            change: |tree| tree.add_local(13, LOCAL_ROOT, "c.txt", "ccc"),
            expected: vec![upload(13, REMOTE_ROOT, "c.txt", "ccc")],
        },
        Case {
            name: "new_remote_file_downloads",
            change: |tree| tree.add_remote("c", "d", "c.txt", "ccc"),
            expected: vec![download("c", 10, "c.txt", "r1")],
        },
        Case {
            name: "unhashed_file_waits",
            change: |tree| {
                tree.add_local(13, LOCAL_ROOT, "c.txt", "ccc");
                tree.local(13).sha1 = None;
            },
            expected: vec![],
        },
        Case {
            name: "local_edit_uploads_revision",
            change: |tree| tree.local(12).sha1 = Some("b2".to_owned()),
            expected: vec![Operation::UploadRevision {
                synced: 3,
                sha1: "b2".to_owned(),
            }],
        },
        Case {
            name: "remote_edit_downloads_revision",
            change: |tree| edited_remotely(tree, "b", "b2"),
            expected: vec![Operation::DownloadRevision {
                synced: 3,
                revision: "r2".to_owned(),
            }],
        },
        Case {
            name: "same_content_revision_refreshes",
            change: |tree| tree.remote("b").revision = Some("r2".to_owned()),
            expected: vec![Operation::Refresh { synced: 3 }],
        },
        Case {
            name: "local_rename_moves_remote",
            change: |tree| tree.local(12).name = "B.txt".to_owned(),
            expected: vec![Operation::MoveRemote {
                synced: 3,
                parent: REMOTE_ROOT.to_owned(),
                name: "B.txt".to_owned(),
            }],
        },
        Case {
            name: "remote_move_moves_local",
            change: |tree| tree.remote("b").parent = "d".to_owned(),
            expected: vec![Operation::MoveLocal {
                synced: 3,
                parent: 10,
                name: "b.txt".to_owned(),
            }],
        },
        Case {
            name: "invalid_remote_name_sanitized",
            change: |tree| tree.remote("b").name = "x/y.txt".to_owned(),
            expected: vec![Operation::MoveLocal {
                synced: 3,
                parent: LOCAL_ROOT,
                name: "x_y.txt".to_owned(),
            }],
        },
        Case {
            name: "conflicting_renames_keep_remote",
            change: |tree| {
                tree.local(12).name = "mine.txt".to_owned();
                tree.remote("b").name = "theirs.txt".to_owned();
            },
            expected: vec![Operation::MoveLocal {
                synced: 3,
                parent: LOCAL_ROOT,
                name: "theirs.txt".to_owned(),
            }],
        },
        Case {
            name: "same_rename_refreshes",
            change: |tree| {
                tree.local(12).name = "same.txt".to_owned();
                tree.remote("b").name = "same.txt".to_owned();
            },
            expected: vec![Operation::Refresh { synced: 3 }],
        },
        Case {
            name: "local_delete_trashes_remote",
            change: |tree| tree.drop_local(12),
            expected: vec![Operation::TrashRemote { synced: 3 }],
        },
        Case {
            name: "remote_delete_trashes_local",
            change: |tree| tree.drop_remote("a"),
            expected: vec![Operation::TrashLocal { synced: 2 }],
        },
        Case {
            name: "local_folder_delete_trashes_once",
            change: |tree| {
                tree.drop_local(10);
                tree.drop_local(11);
            },
            expected: vec![Operation::TrashRemote { synced: 1 }],
        },
        Case {
            name: "local_folder_delete_waits_for_moves",
            change: |tree| {
                tree.drop_local(10);
                tree.local(11).parent = LOCAL_ROOT;
            },
            expected: vec![Operation::MoveRemote {
                synced: 2,
                parent: REMOTE_ROOT.to_owned(),
                name: "a.txt".to_owned(),
            }],
        },
        Case {
            name: "local_folder_delete_revived_by_remote_file",
            change: |tree| {
                tree.drop_local(10);
                tree.drop_local(11);
                tree.add_remote("c", "d", "c.txt", "ccc");
            },
            expected: vec![
                Operation::Forget { synced: 1 },
                Operation::Forget { synced: 2 },
                Operation::CreateLocalFolder {
                    remote: "d".to_owned(),
                    parent: LOCAL_ROOT,
                    name: "docs".to_owned(),
                },
            ],
        },
        Case {
            name: "remote_folder_delete_kept_for_local_edit",
            change: |tree| {
                tree.drop_remote("d");
                tree.drop_remote("a");
                tree.local(11).sha1 = Some("a2".to_owned());
            },
            expected: vec![
                Operation::Forget { synced: 1 },
                Operation::Forget { synced: 2 },
                Operation::CreateRemoteFolder {
                    local: 10,
                    parent: REMOTE_ROOT.to_owned(),
                    name: "docs".to_owned(),
                },
            ],
        },
        Case {
            name: "local_edit_beats_remote_delete",
            change: |tree| {
                tree.drop_remote("b");
                tree.local(12).sha1 = Some("b2".to_owned());
            },
            expected: vec![
                Operation::Forget { synced: 3 },
                upload(12, REMOTE_ROOT, "b.txt", "b2"),
            ],
        },
        Case {
            name: "remote_edit_beats_local_delete",
            change: |tree| {
                tree.drop_local(12);
                edited_remotely(tree, "b", "b2");
            },
            expected: vec![
                Operation::Forget { synced: 3 },
                download("b", LOCAL_ROOT, "b.txt", "r2"),
            ],
        },
        Case {
            name: "edit_conflict_sets_local_aside",
            change: |tree| {
                tree.local(12).sha1 = Some("mine".to_owned());
                edited_remotely(tree, "b", "theirs");
            },
            expected: vec![Operation::SetAside {
                local: 12,
                name: "b (# Edit conflict 2026-10-19 Laptop #).txt".to_owned(),
                conflict: Conflict::Edit,
                forget: Some(3),
            }],
        },
        Case {
            name: "same_edit_refreshes",
            change: |tree| {
                tree.local(12).sha1 = Some("b2".to_owned());
                edited_remotely(tree, "b", "b2");
            },
            expected: vec![Operation::Refresh { synced: 3 }],
        },
        Case {
            name: "same_new_file_links",
            change: |tree| {
                tree.add_local(13, 10, "c.txt", "ccc");
                tree.add_remote("c", "d", "c.txt", "ccc");
            },
            expected: vec![Operation::Link {
                local: 13,
                remote: "c".to_owned(),
            }],
        },
        Case {
            name: "name_clash_sets_local_aside",
            change: |tree| {
                tree.add_local(13, 10, "c.txt", "mine");
                tree.add_remote("c", "d", "c.txt", "theirs");
            },
            expected: vec![Operation::SetAside {
                local: 13,
                name: "c (# Name clash 2026-10-19 Laptop #).txt".to_owned(),
                conflict: Conflict::Clash,
                forget: None,
            }],
        },
        Case {
            name: "normalization_equivalent_names_match",
            change: |tree| {
                tree.add_local(13, LOCAL_ROOT, "caf\u{e9}", "ccc");
                tree.add_remote("c", REMOTE_ROOT, "cafe\u{301}", "ccc");
            },
            expected: vec![Operation::Link {
                local: 13,
                remote: "c".to_owned(),
            }],
        },
        Case {
            name: "replaced_file_rebinds",
            change: |tree| {
                tree.drop_local(12);
                tree.add_local(14, LOCAL_ROOT, "b.txt", "b2");
            },
            expected: vec![Operation::Rebind {
                synced: 3,
                local: Some(14),
                remote: None,
            }],
        },
        Case {
            name: "local_folder_created_before_children",
            change: |tree| {
                tree.local
                    .push(local_item(20, LOCAL_ROOT, "new", Kind::Folder, None));
                tree.add_local(21, 20, "inside.txt", "iii");
            },
            expected: vec![Operation::CreateRemoteFolder {
                local: 20,
                parent: REMOTE_ROOT.to_owned(),
                name: "new".to_owned(),
            }],
        },
        Case {
            name: "cyclic_folder_moves_keep_remote",
            change: |tree| {
                tree.add_synced(
                    4,
                    Kind::Folder,
                    (20, LOCAL_ROOT),
                    ("q", REMOTE_ROOT),
                    "q",
                    None,
                );
                tree.local(10).parent = 20;
                tree.remote("q").parent = "d".to_owned();
            },
            expected: vec![Operation::MoveLocal {
                synced: 1,
                parent: LOCAL_ROOT,
                name: "docs".to_owned(),
            }],
        },
    ]
}

#[test]
fn reconcile_preserves_both_sides() {
    let context = Context::new("Laptop", date(2026, 10, 19));
    for case in cases() {
        let mut tree = Tree::synced();
        (case.change)(&mut tree);
        let plan = reconcile(&tree.state(), &context);
        assert_eq!(plan.operations(), case.expected.as_slice(), "{}", case.name);
    }
}
