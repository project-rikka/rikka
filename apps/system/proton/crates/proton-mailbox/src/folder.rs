use std::collections::HashMap;

use rikka_proton_mail::{Label, LabelCount, LabelId, LabelType, SystemLabel};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Kind {
    Inbox,
    Drafts,
    Scheduled,
    Sent,
    Starred,
    Archive,
    Spam,
    Trash,
    All,
    Folder,
    Label,
}

const STANDING: [(SystemLabel, Kind); 9] = [
    (SystemLabel::Inbox, Kind::Inbox),
    (SystemLabel::Drafts, Kind::Drafts),
    (SystemLabel::AllScheduled, Kind::Scheduled),
    (SystemLabel::Sent, Kind::Sent),
    (SystemLabel::Starred, Kind::Starred),
    (SystemLabel::Archive, Kind::Archive),
    (SystemLabel::Spam, Kind::Spam),
    (SystemLabel::Trash, Kind::Trash),
    (SystemLabel::AllMail, Kind::All),
];

impl Kind {
    #[must_use]
    pub fn is_system(self) -> bool {
        !matches!(self, Kind::Folder | Kind::Label)
    }

    #[must_use]
    pub fn holds_drafts(self) -> bool {
        matches!(self, Kind::Drafts | Kind::Scheduled)
    }

    #[must_use]
    pub fn shows_recipients(self) -> bool {
        matches!(self, Kind::Drafts | Kind::Sent | Kind::Scheduled)
    }

    #[must_use]
    pub fn deletes(self) -> bool {
        matches!(self, Kind::Trash | Kind::Spam | Kind::Drafts)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Folder {
    pub id: LabelId,
    pub kind: Kind,
    pub name: String,
    pub color: String,
    pub depth: usize,
    pub unread: i64,
    pub total: i64,
}

impl Folder {
    fn system(label: SystemLabel, kind: Kind) -> Folder {
        Folder {
            id: label.id(),
            kind,
            name: String::new(),
            color: String::new(),
            depth: 0,
            unread: 0,
            total: 0,
        }
    }

    #[must_use]
    pub fn from_label(label: &Label, depth: usize) -> Folder {
        Folder {
            id: label.id().clone(),
            kind: match label.kind() {
                LabelType::Folder => Kind::Folder,
                _ => Kind::Label,
            },
            name: label.name().to_owned(),
            color: label.color().to_owned(),
            depth,
            unread: 0,
            total: 0,
        }
    }
}

#[must_use]
pub fn arranged(labels: &[Label], counts: &[LabelCount]) -> Vec<Folder> {
    let mut folders: Vec<Folder> = STANDING
        .iter()
        .map(|(label, kind)| Folder::system(*label, *kind))
        .collect();

    let (mut named, mut tagged): (Vec<&Label>, Vec<&Label>) = (Vec::new(), Vec::new());
    for label in labels {
        match label.kind() {
            LabelType::Folder => named.push(label),
            LabelType::Label => tagged.push(label),
            _ => {}
        }
    }

    folders.extend(tree(&named));
    tagged.sort_by_key(|label| (label.order(), label.name().to_lowercase()));
    folders.extend(tagged.into_iter().map(|label| Folder::from_label(label, 0)));

    let known: HashMap<&LabelId, &LabelCount> = counts
        .iter()
        .map(|count| (count.label_id(), count))
        .collect();
    for folder in &mut folders {
        if let Some(count) = known.get(&folder.id) {
            folder.unread = count.unread();
            folder.total = count.total();
        }
    }

    folders
}

fn tree(labels: &[&Label]) -> Vec<Folder> {
    let mut children: HashMap<Option<&LabelId>, Vec<&Label>> = HashMap::new();
    for label in labels {
        children.entry(label.parent_id()).or_default().push(label);
    }
    for siblings in children.values_mut() {
        siblings.sort_by_key(|label| (label.order(), label.name().to_lowercase()));
    }

    let mut arranged = Vec::new();
    branch(&children, None, 0, &mut arranged);

    let placed: Vec<&LabelId> = arranged.iter().map(|folder| &folder.id).collect();
    let orphans: Vec<Folder> = labels
        .iter()
        .filter(|label| !placed.contains(&label.id()))
        .map(|label| Folder::from_label(label, 0))
        .collect();
    arranged.extend(orphans);
    arranged
}

fn branch(
    children: &HashMap<Option<&LabelId>, Vec<&Label>>,
    parent: Option<&LabelId>,
    depth: usize,
    into: &mut Vec<Folder>,
) {
    let Some(siblings) = children.get(&parent) else {
        return;
    };
    for label in siblings {
        into.push(Folder::from_label(label, depth));
        branch(children, Some(label.id()), depth + 1, into);
    }
}
